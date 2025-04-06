mod framed;
pub use framed::{Framed, FramedFilleds, FramedOrders};

mod inspector;
pub use inspector::OrderDetector;

use reth::revm::interpreter::{interpreter::EthInterpreter, InterpreterTypes};
use trevm::{
    helpers::Ctx,
    inspectors::Layered,
    revm::{Database, Inspector},
};

/// Inspector containing an accessible [`OrderDetector`].
pub trait SignetInspector<Ctx, Int = EthInterpreter>: Inspector<Ctx, Int>
where
    Int: InterpreterTypes,
{
    /// Get a reference to the inner [`OrderDetector`].
    fn as_detector(&self) -> &OrderDetector;

    /// Get a mutable reference to the inner [`OrderDetector`].
    fn as_mut_detector(&mut self) -> &mut OrderDetector;
}

impl<Db, T, I, Int> SignetInspector<Ctx<Db>, Int> for Layered<T, I>
where
    Db: Database,
    T: Inspector<Ctx<Db>, Int>,
    I: SignetInspector<Ctx<Db>, Int>,
    Int: InterpreterTypes,
{
    fn as_detector(&self) -> &OrderDetector {
        self.inner().as_detector()
    }

    fn as_mut_detector(&mut self) -> &mut OrderDetector {
        self.inner_mut().as_mut_detector()
    }
}

impl<Db, Int> SignetInspector<Ctx<Db>, Int> for OrderDetector
where
    Int: InterpreterTypes,
    Db: Database,
{
    fn as_detector(&self) -> &OrderDetector {
        self
    }

    fn as_mut_detector(&mut self) -> &mut OrderDetector {
        self
    }
}
