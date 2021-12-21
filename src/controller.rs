use std::sync::mpsc::Sender;

/// ```txt
/// (_before_send<A> before_recv<A>)
///
/// before_send<A> <- A
/// before_recv<A> -> A => B -> after_send<B>
/// after_recv<B> -> B
///
/// (after_send<B> after_recv<B>)
/// ```
pub trait Controller {
    type InputMsg;
    type OutputMsg;

    fn get_connect(&self) -> Sender<Self::InputMsg>;

    fn output(&self) -> Option<Self::OutputMsg> {
        None
    }
}
