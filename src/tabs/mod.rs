pub mod download;
pub mod my_workshop;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Tab {
    MyWorkshop,
    Download,
}
