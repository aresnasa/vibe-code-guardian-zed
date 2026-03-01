use zed_extension_api as zed;

struct VibeCodeGuardian {
    // State will be added here
}

impl VibeCodeGuardian {
    fn new() -> Self {
        Self {}
    }
}

impl zed::Extension for VibeCodeGuardian {
    fn new() -> Self {
        Self::new()
    }
}

zed::register_extension!(VibeCodeGuardian);
