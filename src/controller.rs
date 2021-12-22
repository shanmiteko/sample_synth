pub trait Controller {
    type InputMsg;
    type OutputMsg;

    fn get_connect(&self) -> Self::InputMsg;

    fn output(&self) -> Option<Self::OutputMsg> {
        None
    }
}
