# audio-sender
A simple app that sends audio over the network

## Usage

### Sender
```
audio-streamer sender (v0.1.0)

Usage: sender [OPTIONS]

Options:
  -a, --audio-host <AUDIO_HOST>    Name of the Audio Host
  -d, --device <DEVICE>            Name of the used Audio Device
  -b, --buffer-size <BUFFER_SIZE>  Buffer Size
  -c, --channel <CHANNEL>          Channel Selector
  -e                               Dump Audio Config
  -t <TARGET_SERVER>               Target IP of the server
  -u                               Show Debug TUI
  -p <PORT>                        Port to listen on
  -h, --help                       Print help
  -V, --version                    Print version
```

### Receiver
```
audio-streamer receiver (v0.1.0)

Usage: receiver [OPTIONS]

Options:
  -a, --audio-host <AUDIO_HOST>    Name of the Audio Host
  -d, --device <DEVICE>            Name of the used Audio Device
  -b, --buffer-size <BUFFER_SIZE>  Buffer Size
  -c, --channel <CHANNEL>          Channel Selector
  -e                               Dump Audio Config
  -u                               Show Debug TUI
  -w                               Dump received audio to wav file
  -p <PORT>                        Port to connect to
  -h, --help                       Print help
  -V, --version                    Print version
```