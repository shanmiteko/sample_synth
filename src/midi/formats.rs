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

trait ByteChunk: Sized {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError>;
}

#[derive(Debug)]
pub struct Smf {
    header: HeaderChunk,
    tracks: TrackChunk,
}

impl Smf {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let mut file_buffer = BufReader::new(File::open(path)?);
        Self::read(&mut file_buffer)
    }

    /// microseconds
    ///
    /// [time-division-of-a-midi-file](https://www.recordingblogs.com/wiki/time-division-of-a-midi-file)
    pub fn timebase(&self) -> usize {
        let division = self.header.division;
        let default_tempo: usize = 500_000;
        if division > 0 {
            default_tempo / division as usize
        } else {
            let fps = ((division >> 8) & 0x7F) as usize;
            let tpf = (division & 0x00FF) as usize;
            1_000_000 / (fps * tpf)
        }
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

#[derive(Debug)]
pub enum Format {
    SingleTrack,
    MultipleTrack,
    MultipleSong,
}

#[derive(Debug)]
pub struct TrackChunk {
    tag: Tag,
    track_len: u32,
    track_events: Vec<TrackEvent>,
}

#[derive(Debug)]
pub enum Tag {
    Header,
    Track,
}

#[derive(Debug)]
pub struct TrackEvent {
    delta: U28,
    event: EventKind,
}

#[derive(Debug)]
pub struct U28(u32);

#[derive(Debug)]
pub struct Slice(Vec<u8>);

impl Slice {
    fn to_ascii_str(self) -> String {
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

#[derive(Debug)]
pub enum EventKind {
    Meta { msg: MetaMessage },
    Midi { channel: u8, msg: MidiMessage },
    Sysex { msg: Slice },
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
    EndOfTrack,
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

#[derive(Debug)]
pub struct U7(u8);

pub struct U8(u8, u8);

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
    PitchBend { value: u16 },
}

impl ByteChunk for u8 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 1];
        buf.read(bytes.as_mut())?;
        Ok(u8::from_be_bytes(bytes))
    }
}

impl ByteChunk for u32 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 4];
        buf.read(bytes.as_mut())?;
        Ok(u32::from_be_bytes(bytes))
    }
}

impl ByteChunk for u16 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 2];
        buf.read(bytes.as_mut())?;
        Ok(u16::from_be_bytes(bytes))
    }
}

impl ByteChunk for i16 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let mut bytes = [0u8; 2];
        buf.read(bytes.as_mut())?;
        Ok(i16::from_be_bytes(bytes))
    }
}

impl ByteChunk for Smf {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(Self {
            header: HeaderChunk::read(buf)?,
            tracks: TrackChunk::read(buf)?,
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
            track_events: {
                let mut events = Vec::<TrackEvent>::new();
                while let Ok(track_event) = TrackEvent::read(buf) {
                    match &track_event.event {
                        EventKind::Meta { msg } => match msg {
                            MetaMessage::EndOfTrack => {
                                events.push(track_event);
                                break;
                            }
                            _ => {
                                events.push(track_event);
                            }
                        },
                        _ => {
                            events.push(track_event);
                        }
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
        buf.read(tag.as_mut())?;
        Ok(match &tag {
            b"MThd" => Self::Header,
            b"MTrk" => Self::Track,
            _ => Err(ParseError::UnexpectedTag(tag))?,
        })
    }
}

impl ByteChunk for TrackEvent {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(Self {
            delta: U28::read(buf)?,
            event: EventKind::read(buf)?,
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
        buf.read(slice.as_mut())?;
        Ok(Self(slice))
    }
}

impl ByteChunk for MetaMessage {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        Ok(match u8::read(buf)? {
            0x00 => Self::SequenceNumber(Slice::read(buf)?),
            0x01 => Self::Text(Slice::read(buf)?.to_ascii_str()),
            0x02 => Self::Copyright(Slice::read(buf)?.to_ascii_str()),
            0x03 => Self::TrackName(Slice::read(buf)?.to_ascii_str()),
            0x04 => Self::InstrumentName(Slice::read(buf)?.to_ascii_str()),
            0x05 => Self::Lyric(Slice::read(buf)?.to_ascii_str()),
            0x06 => Self::Marker(Slice::read(buf)?.to_ascii_str()),
            0x07 => Self::CuePoint(Slice::read(buf)?.to_ascii_str()),
            0x20 => Self::ChannelPrefix(Slice::read(buf)?),
            0x2F => Self::EndOfTrack,
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

impl ByteChunk for U8 {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let u_8 = u8::read(buf)?;
        let (high, low) = (u_8 >> 4, u_8 & 0xF);
        Ok(Self(high, low))
    }
}

impl ByteChunk for EventKind {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let U8(high, low) = U8::read(buf)?;
        Ok(match high {
            command if command >= 0x8 && command < 0xF => Self::Midi {
                channel: low,
                msg: match command {
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
                        value: u16::read(buf)?,
                    },
                    _ => unreachable!(),
                },
            },
            0xF => match low {
                0x0 | 0x7 => Self::Sysex {
                    msg: Slice::read(buf)?,
                },
                0xF => Self::Meta {
                    msg: MetaMessage::read(buf)?,
                },
                _ => Err(ParseError::NotSupportedSystemMessage(high))?,
            },
            _ => Err(ParseError::NotCommand(high))?,
        })
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
        let Smf { header, tracks } = Smf::open("test.mid").unwrap();
        // write_log(tracks);
        assert_eq!(format!("{:?}", header.tag), String::from("Header"));
        assert_eq!(format!("{:?}", tracks.tag), String::from("Track"));
    }

    #[test]
    fn u28() {
        let mut bytes = get_buf([0x82u8, 0x80, 0x00, 0xff].as_ref());
        let U28(inner) = U28::read(&mut bytes).unwrap();
        assert_eq!(inner, 32768);
        let mut bytes = get_buf([0x81u8, 0x7f, 0xff].as_ref());
        let U28(inner) = U28::read(&mut bytes).unwrap();
        assert_eq!(inner, 255);
    }

    #[test]
    fn slice_to_string() {
        let ascii = br"abcdef ghijklmnop";
        let ascii_vec = Vec::from(&ascii[..]);
        let ascii_str = String::from("abcdef ghijklmnop");
        assert_eq!(Slice(ascii_vec).to_ascii_str(), ascii_str);
    }

    #[test]
    fn slice_to_u32() {
        let nums = [0x07u8, 0xA1, 0x20];
        let nums_vec = Vec::from(&nums[..]);
        assert_eq!(Slice(nums_vec).to_u32(), 500000)
    }

    #[test]
    fn u8_split_to_low_and_high() {
        let mut bytes = get_buf([0x81u8].as_ref());
        let U8(l, r) = U8::read(&mut bytes).unwrap();
        assert_eq!(l, 8);
        assert_eq!(r, 1);
    }
}
