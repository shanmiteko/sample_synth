use super::formats::{Format, Smf};

struct Player {}

impl Player {
    fn new(smf: Smf) -> Self {
        match smf.format() {
            Format::SingleTrack => todo!(),
            Format::MultipleTrack => todo!(),
            Format::MultipleSong => todo!(),
        }
    }

    fn play() {
        todo!()
    }
}
