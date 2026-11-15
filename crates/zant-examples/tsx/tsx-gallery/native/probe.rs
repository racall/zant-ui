pub struct Native;
impl zant::tsx::native::NativeModule for Native { const NAME: &'static str = "probe"; }
impl crate::native::spec::probe::ProbeSpec for Native {
    fn label() -> zant::Str { zant::Str::Static("Gallery native baseline") }
}
