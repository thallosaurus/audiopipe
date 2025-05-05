# uncomplicated-audio-streamer
A simple app that sends audio over the network with the aim of realtime, low-latency performance

## Usage

### Sender
```
uastreamer (v0.2.2)

Usage: sender [OPTIONS]

Options:
  -n, --network-host <NETWORK_HOST>  Name of the Audio Host
  -a, --audio-host <AUDIO_HOST>      Name of the Audio Host
  -d, --device <DEVICE>              Name of the used Audio Device
  -b, --buffer-size <BUFFER_SIZE>    Buffer Size
  -p <PORT>                          Port to connect to
  -c <OUTPUT_CHANNELS>               Selects the tracks to which the app will push data to
  -h, --help                         Print help
  -V, --version                      Print version
```

### Receiver
```
uastreamer (v0.2.2)

Usage: receiver [OPTIONS]

Options:
  -n, --network-host <NETWORK_HOST>  Name of the Audio Host
  -a, --audio-host <AUDIO_HOST>      Name of the Audio Host
  -d, --device <DEVICE>              Name of the used Audio Device
  -b, --buffer-size <BUFFER_SIZE>    Buffer Size
  -p <PORT>                          Port to connect to
  -c <OUTPUT_CHANNELS>               Selects the tracks to which the app will push data to
  -h, --help                         Print help
  -V, --version                      Print version
```
