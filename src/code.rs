use crate::builder::Builder;
use crate::tape::UnexpectedEndError;

pub trait Build<Cpu, Ram> {
    type Error: From<UnexpectedEndError>;

    fn build<'tape, 'code>(
        &'code self,
        builder: &mut Builder<'tape, 'code, Cpu, Ram>,
    ) -> Result<(), Self::Error>
    where
        'code: 'tape;
}
