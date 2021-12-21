use crate::controller::Controller;
use std::sync::mpsc::{self, Receiver, Sender};

const MIDDLE_C: f64 = 440.0;
const NEGATIVE_C: f64 = MIDDLE_C / 32_f64;

struct MidiControl<I> {
    input: (Sender<I>, Receiver<I>),
}

impl<I> MidiControl<I> {
    fn new() -> Self {
        let input = mpsc::channel::<I>();
        Self { input }
    }
}

impl Controller for MidiControl<(bool, u8)> {
    type InputMsg = (bool, u8);
    type OutputMsg = MidiMessage;

    fn get_connect(&self) -> Sender<Self::InputMsg> {
        self.input.0.clone()
    }

    fn output(&self) -> Option<Self::OutputMsg> {
        self.input.1.recv().ok().map(|(on, code)| {
            if on {
                MidiMessage::NoteOn(KeyCode(code))
            } else {
                MidiMessage::NoteOff(KeyCode(code))
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct KeyCode(u8);

impl KeyCode {
    const MIN: u8 = 0;
    const MAX: u8 = 127;

    fn as_hz(self) -> f64 {
        NEGATIVE_C * 2_f64.powf(f64::from(self.0) / 12_f64)
    }
}

struct VariableLenVal {
    inner: Vec<u8>,
}

impl VariableLenVal {
    fn new(inner: Vec<u8>) -> Self {
        Self { inner }
    }

    fn to_real_value(mut self) -> u16 {
        let len = self.inner.len();
        let mut total = 0u16;

        for (index, byte) in self.inner.iter_mut().enumerate() {
            total += (*byte as u16) << ((len - index - 1) * 8);
        }
        total
    }
}

enum MidiMessage {
    NoteOn(KeyCode),
    NoteOff(KeyCode),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};
    #[test]
    fn key_to_f64_freq() {
        let c4 = KeyCode(60);
        let d4 = KeyCode(62);
        assert!(c4.as_hz() - 440.0 < 0.01);
        assert!(d4.as_hz() - 493.88 < 0.01);
    }

    #[test]
    fn midi_in_out() {
        let midictl = MidiControl::<(bool, u8)>::new();
        let midictl_conn = midictl.get_connect();
        let press_code = thread::spawn(move || {
            // press c4
            midictl_conn.send((true, 60)).unwrap();
            thread::sleep(Duration::from_millis(100));
            midictl_conn.send((false, 60)).unwrap();
            // wait
            thread::sleep(Duration::from_millis(200));
            // press c5
            midictl_conn.send((true, 61)).unwrap();
            thread::sleep(Duration::from_millis(100));
            midictl_conn.send((false, 61)).unwrap();
        });
        let _listener = thread::spawn(move || {
            while let Some(msg) = midictl.output() {
                println!(
                    "{}",
                    match msg {
                        MidiMessage::NoteOn(code) => format!("on {}", code.0),
                        MidiMessage::NoteOff(code) => format!("off {}", code.0),
                    }
                )
            }
        });
        press_code.join().unwrap();
    }

    #[test]
    fn variablelenval_to_real_value() {
        let vlv = VariableLenVal::new(vec![82, 80, 00]);
        assert_eq!(vlv.to_real_value(), 32768);
    }
}
