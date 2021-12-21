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
    fn open<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let mut file_buffer = BufReader::new(File::open(path)?);
        Self::read(&mut file_buffer)
    }
}

#[derive(Debug)]
pub struct HeaderChunk {
    tag: Tag,
    header_len: u32,
    format: Format,
    track_num: u16,
    timebase: i16,
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
pub enum EventKind {
    Meta {
        meta_type: MetaType,
        data_len: U28,
        data: Vec<u8>,
    },
    Midi {
        channel: u8,
        msg: MidiMessage,
    },
    Sysex {
        data_len: U28,
        data: Vec<u8>,
    },
}

#[derive(Debug)]
pub enum MetaType {
    TrackNumber,
    Text,
    Copyright,
    TrackName,
    InstrumentName,
    Lyric,
    Marker,
    CuePoint,
    ProgramName,
    DeviceName,
    MidiChannel,
    MidiPort,
    EndOfTrack,
    Tempo,
    SmpteOffset,
    TimeSignature,
    KeySignature,
    SequencerSpecific,
    Unknown,
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
            timebase: i16::read(buf)?,
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
                while let Ok(te) = TrackEvent::read(buf) {
                    events.push(te);
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
                inner += u_8 as u32;
            } else {
                inner += u_8 as u32;
                break;
            }
        }
        Ok(Self(inner))
    }
}

impl ByteChunk for MetaType {
    fn read<B: BufRead>(buf: &mut B) -> Result<Self, ParseError> {
        let u_8 = u8::read(buf)?;
        Ok(match u_8 {
            0x00 => Self::TrackNumber,
            0x01 => Self::Text,
            0x02 => Self::Copyright,
            0x03 => Self::TrackName,
            0x04 => Self::InstrumentName,
            0x05 => Self::Lyric,
            0x06 => Self::Marker,
            0x07 => Self::CuePoint,
            0x08 => Self::ProgramName,
            0x09 => Self::DeviceName,
            0x20 => Self::MidiChannel,
            0x21 => Self::MidiPort,
            0x2F => Self::EndOfTrack,
            0x51 => Self::Tempo,
            0x54 => Self::SmpteOffset,
            0x58 => Self::TimeSignature,
            0x59 => Self::KeySignature,
            0x7F => Self::SequencerSpecific,
            _ => Self::Unknown,
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
                0x0 | 0x7 => {
                    let data_len = U28::read(buf)?;
                    let mut data = vec![0; data_len.0 as usize];
                    buf.read(data.as_mut())?;
                    Self::Sysex { data_len, data }
                }
                0xF => {
                    let meta_type = MetaType::read(buf)?;
                    let data_len = U28::read(buf)?;
                    let mut data = vec![0; data_len.0 as usize];
                    buf.read(data.as_mut())?;
                    Self::Meta {
                        meta_type,
                        data_len,
                        data,
                    }
                }
                _ => Err(ParseError::NotSupportedSystemMessage(high))?,
            },
            _ => Err(ParseError::NotCommand(high))?,
        })
    }
}

#[cfg(test)]
mod midi_tests {
    use std::io::{BufReader, Read};

    use super::*;

    fn get_buf<B: Read>(bytes: B) -> BufReader<B> {
        BufReader::new(bytes)
    }

    #[test]
    fn parse() {
        let _ = Smf::open("null.mid").unwrap();
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
    fn u8_split_to_low_and_high() {
        let mut bytes = get_buf([0x81u8].as_ref());
        let U8(l, r) = U8::read(&mut bytes).unwrap();
        assert_eq!(l, 8);
        assert_eq!(r, 1);
    }
}
