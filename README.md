# Simple Synthesis

## Todo
- [ ] Oscillator
  - [ ] Sine
  - [ ] Sawtooth
  - [ ] Triangle
  - [ ] Square
  - [ ] Noise
- [ ] Filter
  - [ ] low pass
  - [ ] high pass
- [ ] Envelope
  - [ ] ADSR
- [ ] Effect
- [ ] Midi
  - [x] parse file
  - [ ] play

## How to Build

### Requirement
```bash
$ sudo zypper install alsa-devel
$ cargo build --release
```

### Reference
* [Frame](https://alsa.opensrc.org/Frame)
* [PCM / WAV 格式](https://www.cnblogs.com/renhui/p/12148330.html)
* [What-is-MIDI](https://www.instructables.com/What-is-MIDI/)
* [MIDI文件格式解析](https://www.jianshu.com/p/59d74800b43b)
* [Outline of the Standard MIDI File Structure](http://www.ccarh.org/courses/253/handout/smf/)
* [MIDI Communication Protocol](http://www.ccarh.org/courses/253/handout/midiprotocol/)
* [Variable Length Values ](http://www.ccarh.org/courses/253/handout/vlv/)
* [midly](https://github.com/negamartin/midly)
