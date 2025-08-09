/// Holds everything related to the TCP Communication Channel
pub mod control;

/// Holds all things streamer related
//pub mod streamer;

pub mod cpal;

pub mod udp;

use ::cpal::{traits::StreamTrait, SizedSample};
use log::{debug, error, info};

use std::{net::SocketAddr, sync::{mpsc::{self, channel, Receiver, Sender}, Arc, Mutex}};

use bytemuck::Pod;
use ringbuf::{traits::{Observer, Split}, HeapCons, HeapProd};
use threadpool::ThreadPool;

use crate::{config::{get_cpal_config, StreamerConfig}, pooled::{control::{TcpControlFlow, TcpErrors, TcpResult}, cpal::{CpalAudioFlow, CpalStats, CpalStatus}, udp::{NetworkUDPStats, UdpStatus, UdpStreamFlow}}};


/// Struct that holds everything the library needs
#[deprecated]
pub struct App<T: SizedSample + Send + Pod + Default + std::fmt::Debug + 'static> {
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
    cpal_stats_sender: Sender<CpalStats>,
    udp_stats_sender: Sender<NetworkUDPStats>,
    udp_command_sender: Option<Sender<UdpStatus>>,
    cpal_command_sender: Option<Sender<CpalStatus>>,

    _config: Arc<Mutex<StreamerConfig>>,
    pub pool: ThreadPool,
    //thread_channels: Option<(Sender<bool>, Sender<bool>)>,
}

impl<T> App<T>
where
    T: SizedSample + Send + Pod + Default + std::fmt::Debug + 'static,
{
    pub fn new(config: StreamerConfig) -> (Self, AppDebug) {
        debug!("Using Config: {:?}", &config);
        let audio_buffer =
            ringbuf::HeapRb::<T>::new(config.buffer_size * config.selected_channels.len());
        info!("Audio Buffer Size: {}", audio_buffer.capacity());

        let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();

        let (cpal_stats_sender, cpal_stats_receiver) = mpsc::channel::<CpalStats>();
        let (udp_stats_sender, udp_stats_receiver) = mpsc::channel::<NetworkUDPStats>();

        (
            Self {
                audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
                audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
                cpal_stats_sender,
                udp_stats_sender,
                _config: Arc::new(Mutex::new(config)),
                pool: ThreadPool::new(5),
                udp_command_sender: None,
                cpal_command_sender: None,
                //thread_channels: None,
            },
            AppDebug {
                cpal_stats_receiver,
                udp_stats_receiver,
            },
        )
    }
}

#[deprecated]
pub struct AppDebug {
    cpal_stats_receiver: Receiver<CpalStats>,
    udp_stats_receiver: Receiver<NetworkUDPStats>,
}

impl<T: SizedSample + Send + Pod + Default + std::fmt::Debug + 'static> CpalAudioFlow<T> for App<T> {
    fn get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }

    fn get_cpal_stats_sender(&self) -> Sender<CpalStats> {
        self.cpal_stats_sender.clone()
    }
}

impl<T: SizedSample + Send + Pod + Default + std::fmt::Debug + 'static> UdpStreamFlow<T> for App<T> {
    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }

    fn get_udp_stats_sender(&self) -> Sender<NetworkUDPStats> {
        self.udp_stats_sender.clone()
    }
}

impl<T> TcpControlFlow for App<T>
where
    T: SizedSample + Send + Pod + Default + std::fmt::Debug + 'static,
{
    fn start_stream(
        &mut self,
        config: StreamerConfig,
        target: SocketAddr,
    ) -> TcpResult<()> {
        // Sync Channel for the buffer
        // If sent, the udp thread is instructed to empty the contents of the buffer and send them
        let (chan_sync_tx, chan_sync_rx) = channel::<bool>();

        let (cpal_channel_tx, cpal_channel_rx) = channel::<CpalStatus>();
        let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        self.udp_command_sender = Some(udp_msg_tx);
        self.cpal_command_sender = Some(cpal_channel_tx);

        //start video capture and udp sender here
        let dir = config.direction;
        {
            info!("Constructing CPAL Stream");
            let stats = self.get_cpal_stats_sender();
            let prod = self.get_producer();
            let cons = self.get_consumer();
            let streamer_config = config.clone();

            self.pool.execute(move || {
                let (d, stream_config) = get_cpal_config(
                    config.direction,
                    config.program_args.audio_host,
                    config.program_args.device,
                )
                .unwrap();

                //let device = device.lock().unwrap();
                match Self::construct_stream(
                    dir,
                    &d,
                    stream_config,
                    streamer_config,
                    stats,
                    prod,
                    cons,
                    chan_sync_tx,
                ) {
                    Ok(stream) => {
                        stream.play().unwrap();
                        loop {
                            //block
                            if let Ok(msg) = cpal_channel_rx.try_recv() {
                                match msg {
                                    CpalStatus::DidEnd => {
                                        println!("Stopping CPAL Stream");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => error!("{}", err),
                }
            });
        }

        {
            info!("Constructing UDP Stream");
            let prod = self.udp_get_producer();
            let cons = self.udp_get_consumer();
            let stats = self.get_udp_stats_sender();
            debug!("{:?}", target);

            let t = target.clone();
            //t.set_ip(IpAddr::from_str("0.0.0.0").unwrap());
            //t.set_port(42069);let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

            self.pool.execute(move || {
                Self::construct_udp_stream(
                    dir,
                    t,
                    cons,
                    prod,
                    Some(stats),
                    udp_msg_rx,
                    chan_sync_rx,
                )
                .unwrap();
            });
        }
        Ok(())
    }

    fn stop_stream(&self) -> TcpResult<()> {
        if let Some(ch) = &self.udp_command_sender {
            ch.send(UdpStatus::DidEnd).map_err(|e| {
                TcpErrors::UdpStatsSendError(e)
            })?;
        }

        if let Some(ch) = &self.cpal_command_sender {
            ch.send(CpalStatus::DidEnd).map_err(|e| {
                TcpErrors::CpalStatsSendError(e)
            })?;
        }

        Ok(())
    }
}