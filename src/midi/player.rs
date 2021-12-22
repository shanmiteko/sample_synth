use super::formats::Smf;

struct Player {}

impl Player {
    fn new(smf: Smf) -> Self {
        let mut timebase = smf.timebase();
        todo!()
    }

    fn play() {}
}
