# audiopipe
A simple app that sends audio over the network with the aim of realtime, low-latency performance

## Usage
```
audiopipe (v0.3.0)

Usage: audiopipe [OPTIONS] -t <TRACK_SELECTOR> <COMMAND>

Commands:
  sender    Start the application in sender mode
  receiver  Start the application in receiver mode
  devices   Enumerate audio device hardware
  help      Print this message or the help of the given subcommand(s)

Options:
  -a, --audio-host <AUDIO_HOST>    Name of the Audio Host to be used (default: Use system default)
  -d, --device <DEVICE>            Name of the Audio Device to be used (default: Use system default)
  -b, --buffer-size <BUFFER_SIZE>  Requested Device Buffer Size
  -s, --samplerate <SAMPLERATE>    Requested Sample Rate
  -t <TRACK_SELECTOR>              Selects which tracks are the master inputs/outputs. Use "-t 0" for mono and "-t 0,1" for stereo
  -h, --help                       Print help
  -V, --version                    Print version
```