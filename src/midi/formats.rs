use std::{
    fs::File,
    io::{BufRead, BufReader, Error as StdIoError},
    path::Path,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("midi buffer parse error")]
    IOError(#[from] StdIoError),
    #[error("unexpected tag `{0:?}`")]
    UnexpectedTag([u8; 4]),
    #[error("unexpected format `{0}`")]
    UnexpectedFormat(u16),
    #[error("not command `{0}` should < 8")]
    NotCommand(u8),
    #[error("not data `{0}` should >= 8")]
    NotData(u8),
    #[error("not supported system message `{0}`")]
    NotSupportedSystemMessage(u8),
}

impl ParseError {
    pub fn is_eof(&self) -> bool {
        match self {
            Self::IOError(ioe) => match ioe.kind() {
                std::io::ErrorKind::UnexpectedEof => true,
                _ => false,
            },
            _ => false,
        }
    }
}

trait ByteChunk: Sized {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError>;
}

#[derive(Debug)]
pub struct Smf {
    header: HeaderChunk,
    tracks: Vec<TrackChunk>,
}

impl Smf {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let mut file_buffer = BufReader::new(File::open(path)?);
        Self::read(&mut file_buffer)
    }

    /// Returns the number of milliseconds at 1 tick
    ///
    /// [time-division-of-a-midi-file](https://www.recordingblogs.com/wiki/time-division-of-a-midi-file)
    pub fn timebase(&self) -> usize {
        let division = self.header.division;
        let default_tempo: usize = 500_000;
        if division > 0 {
            default_tempo / division as usize
        } else {
            let fps = ((division >> 8) & 0x007F) as usize;
            let tpf = (division & 0x00FF) as usize;
            1_000_000 / (fps * tpf)
        }
    }

    /// Returns format type and number of tracks
    pub fn format(&self) -> Format {
        self.header.format
    }

    pub fn tracks(&self) -> &Vec<TrackChunk> {
        &self.tracks
    }
}

#[derive(Debug)]
pub struct HeaderChunk {
    tag: Tag,
    header_len: u32,
    format: Format,
    track_num: u16,
    /// if division > 0: Pulses per quarter note
    ///
    /// else: Or Frames per second
    division: i16,
}

impl HeaderChunk {
    pub fn track_num(&self) -> u16 {
        self.track_num
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Format {
    SingleTrack,
    MultipleTrack,
    MultipleSong,
}

#[derive(Debug)]
pub struct TrackChunk {
    tag: Tag,
    track_len: u32,
    events: Vec<TrackEvent>,
}

#[derive(Debug)]
pub enum Tag {
    Header,
    Track,
}

#[derive(Debug)]
pub struct TrackEvent {
    delta: U28,
    event: Event,
}

impl TrackEvent {
    /// Returns tick and event pair
    pub fn event(&self) -> (u32, &Event) {
        (self.delta.0, &self.event)
    }
}

/// Variable Length Values
#[derive(Debug)]
pub struct U28(u32);

/// U28 + U28 * u8
#[derive(Debug)]
pub struct Slice(Vec<u8>);

impl Slice {
    fn to_ascii(self) -> String {
        self.0
            .into_iter()
            .map(|c| {
                #[inline]
                fn hexify(b: u8) -> u8 {
                    match b {
                        0..=9 => b'0' + b,
                        _ => b'a' + b - 10,
                    }
                }

                let (data, len) = match c {
                    b'\t' => ([b'\\', b't', 0, 0], 2),
                    b'\r' => ([b'\\', b'r', 0, 0], 2),
                    b'\n' => ([b'\\', b'n', 0, 0], 2),
                    b'\\' => ([b'\\', b'\\', 0, 0], 2),
                    b'\'' => ([b'\\', b'\'', 0, 0], 2),
                    b'"' => ([b'\\', b'"', 0, 0], 2),
                    b'\x20'..=b'\x7e' => ([c, 0, 0, 0], 1),
                    _ => ([b'\\', b'x', hexify(c >> 4), hexify(c & 0xf)], 4),
                };

                unsafe { String::from_utf8_unchecked(Vec::from(&data[0..len])) }
            })
            .collect::<String>()
    }

    fn to_u32(self) -> u32 {
        let mut num = 0u32;
        let nums = self.0;
        for i in 0..4 {
            if let Some(n) = nums.get(i) {
                num <<= 8;
                num += *n as u32;
            }
        }
        num
    }
}

/// ```txt
/// Midi: delta(U28)+status(u4>7)+channel(u4)+data
/// Meta: delta(U28)+status(0xFF)+type(u8)+U28+U28*u8
/// Sysex: delta(U28)+status(0xF0|0xF7)+data(n*u8)+end(0xF7)
/// if status < 8: omit status; status = previous status
/// ```
/// - [midi-event](https://www.recordingblogs.com/wiki/midi-event)
/// - [status-byte-of-a-midi-message](https://www.recordingblogs.com/wiki/status-byte-of-a-midi-message)
#[derive(Debug)]
pub enum Event {
    Meta { meta_msg: MetaMessage },
    Midi { channel: u8, midi_msg: MidiMessage },
    Sysex { sysex_msg: Slice },
}

impl Event {
    pub fn is_end(&self) -> bool {
        match &self {
            Event::Meta { meta_msg } => match meta_msg {
                MetaMessage::EndOfTrack(_) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum MetaMessage {
    SequenceNumber(Slice),
    Text(String),
    Copyright(String),
    TrackName(String),
    InstrumentName(String),
    Lyric(String),
    Marker(String),
    CuePoint(String),
    ChannelPrefix(Slice),
    EndOfTrack(Slice),
    /// value 0x07A120 (500000 decimal) means that there are 500,000 microseconds per quarter note.
    ///
    /// Since there are 60,000,000 microseconds per minute,
    ///
    /// the message above translates to:
    ///
    /// set the tempo to 60,000,000 / 500,000 = 120 quarter notes per minute (120 beats per minute).
    ///
    /// [midi-set-tempo-meta-message](https://www.recordingblogs.com/wiki/midi-set-tempo-meta-message)
    Tempo(u32),
    SmpteOffset(Slice),
    TimeSignature(Slice),
    KeySignature(Slice),
    SequencerSpecific(Slice),
    Unknown(Slice),
}

/// Midi data < 80
#[derive(Debug)]
pub struct U7(u8);

/// Little endian and removing the top-most bit of each byte
///
/// [midi-pitch-wheel-message](https://www.recordingblogs.com/wiki/midi-pitch-wheel-message)
#[derive(Debug)]
pub struct U14(u16);

#[derive(Debug)]
pub enum MidiMessage {
    /// Stop playing a note.
    NoteOff { key: U7, vel: U7 },
    /// Start playing a note.
    NoteOn { key: U7, vel: U7 },
    /// Modify the velocity of a note after it has been played.
    Aftertouch { key: U7, vel: U7 },
    /// Modify the value of a MIDI controller.
    ControlChange { controller: U7, value: U7 },
    /// Change the program (also known as instrument) for a channel.
    PatchChange { program: U7 },
    /// Change the note velocity of a whole channel at once, without starting new notes.
    ChannelPressure { vel: U7 },
    /// Set the pitch bend value for the entire channel.
    PitchBend { value: U14 },
}

impl ByteChunk for u8 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 1];
        buf.read_exact(bytes.as_mut())?;
        Ok(u8::from_be_bytes(bytes))
    }
}

impl ByteChunk for u32 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 4];
        buf.read_exact(bytes.as_mut())?;
        Ok(u32::from_be_bytes(bytes))
    }
}

impl ByteChunk for u16 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 2];
        buf.read_exact(bytes.as_mut())?;
        Ok(u16::from_be_bytes(bytes))
    }
}

impl ByteChunk for i16 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 2];
        buf.read_exact(bytes.as_mut())?;
        Ok(i16::from_be_bytes(bytes))
    }
}

impl ByteChunk for Smf {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(Self {
            header: HeaderChunk::read(buf)?,
            tracks: {
                let mut tracks = Vec::<TrackChunk>::new();
                loop {
                    match TrackChunk::read(buf) {
                        Ok(track) => {
                            tracks.push(track);
                        }
                        Err(pe) => {
                            if pe.is_eof() {
                                break;
                            } else {
                                Err(pe)?
                            }
                        }
                    }
                }
                tracks
            },
        })
    }
}

impl ByteChunk for HeaderChunk {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(Self {
            tag: Tag::read(buf)?,
            header_len: u32::read(buf)?,
            format: Format::read(buf)?,
            track_num: u16::read(buf)?,
            division: i16::read(buf)?,
        })
    }
}

impl ByteChunk for Format {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let format = u16::read(buf)?;
        Ok(match format {
            0 => Self::SingleTrack,
            1 => Self::MultipleTrack,
            2 => Self::MultipleSong,
            _ => Err(ParseError::UnexpectedFormat(format))?,
        })
    }
}

impl ByteChunk for TrackChunk {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(Self {
            tag: Tag::read(buf)?,
            track_len: u32::read(buf)?,
            events: {
                let mut events = Vec::<TrackEvent>::new();
                let mut previous_status = 0u8;
                loop {
                    let delta = U28::read(buf)?;
                    let mut new_status = buf.fill_buf()?[0];
                    if new_status < 0x80 {
                        if previous_status < 0x80 {
                            Err(ParseError::NotCommand(new_status))?
                        } else {
                            new_status = previous_status;
                        }
                    } else {
                        previous_status = new_status;
                        buf.consume(1);
                    }
                    let (high, low) = (new_status >> 4, new_status & 0xF);
                    let event = match high {
                        command if command >= 0x8 && command < 0xF => Event::Midi {
                            channel: low,
                            midi_msg: match command {
                                0x8 => MidiMessage::NoteOff {
                                    key: U7::read(buf)?,
                                    vel: U7::read(buf)?,
                                },
                                0x9 => MidiMessage::NoteOn {
                                    key: U7::read(buf)?,
                                    vel: U7::read(buf)?,
                                },
                                0xA => MidiMessage::Aftertouch {
                                    key: U7::read(buf)?,
                                    vel: U7::read(buf)?,
                                },
                                0xB => MidiMessage::ControlChange {
                                    controller: U7::read(buf)?,
                                    value: U7::read(buf)?,
                                },
                                0xC => MidiMessage::PatchChange {
                                    program: U7::read(buf)?,
                                },
                                0xD => MidiMessage::ChannelPressure {
                                    vel: U7::read(buf)?,
                                },
                                0xE => MidiMessage::PitchBend {
                                    value: U14::read(buf)?,
                                },
                                _ => unreachable!(),
                            },
                        },
                        0xF => match low {
                            0x0 | 0x7 => Event::Sysex {
                                sysex_msg: Slice::read(buf)?,
                            },
                            0xF => Event::Meta {
                                meta_msg: MetaMessage::read(buf)?,
                            },
                            _ => Err(ParseError::NotSupportedSystemMessage(new_status))?,
                        },
                        _ => unreachable!(),
                    };
                    let is_end = event.is_end();
                    events.push(TrackEvent { delta, event });
                    if is_end {
                        break;
                    }
                }
                events
            },
        })
    }
}

impl ByteChunk for Tag {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut tag = [0u8; 4];
        buf.read_exact(tag.as_mut())?;
        Ok(match &tag {
            b"MThd" => Self::Header,
            b"MTrk" => Self::Track,
            _ => Err(ParseError::UnexpectedTag(tag))?,
        })
    }
}

impl ByteChunk for U28 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut inner = 0u32;
        for _ in 0..4 {
            let mut u_8 = u8::read(buf)?;
            inner <<= 7;
            if u_8 >= 0x80 {
                u_8 &= 0x7F;
                inner += u32::from(u_8);
            } else {
                inner += u32::from(u_8);
                break;
            }
        }
        Ok(Self(inner))
    }
}

impl ByteChunk for Slice {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let data_len = U28::read(buf)?;
        let mut slice = vec![0u8; data_len.0 as usize];
        buf.read_exact(slice.as_mut())?;
        Ok(Self(slice))
    }
}

impl ByteChunk for MetaMessage {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(match u8::read(buf)? {
            0x00 => Self::SequenceNumber(Slice::read(buf)?),
            0x01 => Self::Text(Slice::read(buf)?.to_ascii()),
            0x02 => Self::Copyright(Slice::read(buf)?.to_ascii()),
            0x03 => Self::TrackName(Slice::read(buf)?.to_ascii()),
            0x04 => Self::InstrumentName(Slice::read(buf)?.to_ascii()),
            0x05 => Self::Lyric(Slice::read(buf)?.to_ascii()),
            0x06 => Self::Marker(Slice::read(buf)?.to_ascii()),
            0x07 => Self::CuePoint(Slice::read(buf)?.to_ascii()),
            0x20 => Self::ChannelPrefix(Slice::read(buf)?),
            0x2F => Self::EndOfTrack(Slice::read(buf)?),
            0x51 => Self::Tempo(Slice::read(buf)?.to_u32()),
            0x54 => Self::SmpteOffset(Slice::read(buf)?),
            0x58 => Self::TimeSignature(Slice::read(buf)?),
            0x59 => Self::KeySignature(Slice::read(buf)?),
            0x7F => Self::SequencerSpecific(Slice::read(buf)?),
            _ => Self::Unknown(Slice::read(buf)?),
        })
    }
}

impl ByteChunk for U7 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let u_8 = u8::read(buf)?;
        if u_8 >= 0x80 {
            Err(ParseError::NotData(u_8))?
        } else {
            Ok(U7(u_8))
        }
    }
}

impl ByteChunk for U14 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut inner = 0u16;
        let (high, low) = (u8::read(buf)?, u8::read(buf)?);
        inner += u16::from(low & 0x7F);
        inner <<= 7;
        inner += u16::from(high & 0x7F);
        Ok(Self(inner))
    }
}

#[cfg(test)]
mod midi_tests {
    use std::{
        fmt::Debug,
        io::{BufReader, Read, Write},
    };

    use super::*;

    fn get_buf<B: Read>(bytes: B) -> BufReader<B> {
        BufReader::new(bytes)
    }

    #[allow(unused)]
    fn write_log<T: Debug>(any: T) {
        let mut file = File::create("0.log").unwrap();
        write!(file, "{:#?}", any).unwrap();
    }

    #[test]
    fn parse() {
        let Smf { header, tracks } = Smf::open("sandstorm.mid").unwrap();
        // write_log(header);
        // write_log(tracks);
        assert_eq!(header.track_num as usize, tracks.len());
    }

    #[test]
    fn struct_u14() {
        let mut bytes = get_buf([0x54u8, 0x39].as_ref());
        let U14(inner) = U14::read(&mut bytes).unwrap();
        assert_eq!(inner, 0x1CD4);
        let mut bytes = get_buf([0x01u8, 0x01].as_ref());
        let U14(inner) = U14::read(&mut bytes).unwrap();
        assert_eq!(inner, 0x81);
    }

    #[test]
    fn struct_u28() {
        let mut bytes = get_buf([0x82u8, 0x80, 0x00, 0xff].as_ref());
        let U28(inner) = U28::read(&mut bytes).unwrap();
        assert_eq!(inner, 32768);
        let mut bytes = get_buf([0x81u8, 0x7f, 0xff].as_ref());
        let U28(inner) = U28::read(&mut bytes).unwrap();
        assert_eq!(inner, 255);
    }

    #[test]
    fn struct_slice() {
        let mut bytes = get_buf([0x00u8].as_ref());
        assert_eq!(Slice::read(&mut bytes).unwrap().0, vec![]);
        let mut bytes = get_buf([0x01u8, 0x01u8].as_ref());
        assert_eq!(Slice::read(&mut bytes).unwrap().0, vec![01u8]);
        let mut bytes = get_buf([0x02u8, 0x01u8, 0x02].as_ref());
        assert_eq!(Slice::read(&mut bytes).unwrap().0, vec![0x01u8, 0x02]);
    }

    #[test]
    fn slice_to_string() {
        let ascii = br"abcdef ghijklmnop";
        let ascii_vec = Vec::from(&ascii[..]);
        let ascii_str = String::from("abcdef ghijklmnop");
        assert_eq!(Slice(ascii_vec).to_ascii(), ascii_str);
    }

    #[test]
    fn slice_to_u32() {
        let vec = vec![0x07u8, 0xA1, 0x20];
        assert_eq!(Slice(vec).to_u32(), 500000)
    }
}
