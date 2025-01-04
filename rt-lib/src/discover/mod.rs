//! Mac is searched at /Library/Frameworks
mod linux;
mod mac;
pub use mac::*;
mod windows;
pub use linux::*;
pub use windows::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RVersion;

    #[test]
    fn discover_default() {
        let default = RVersion::default();
        println!("{:?}", default);
    }

    #[test]
    fn discover_mac_() {
        let discovered = discover_mac();
        println!("{:#?}", discovered);
    }

    #[test]
    fn discover_linux_() {
        let discovered = crate::discover::discover_linux();
        println!("{:?}", discovered);
    }

    #[test]
    fn discover_windows_() {
        let discovered = crate::discover::discover_windows();
        println!("{:?}", discovered);
    }

    #[test]
    fn discover() {
        use crate::RVersions;
        println!("{:?}", RVersions::discover());
    }
}
