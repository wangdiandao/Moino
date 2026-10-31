use typst::{
    Library, LibraryExt, World,
    diag::{FileError, FileResult},
    foundations::{Bytes, Datetime},
    syntax::{FileId, Source},
    text::Font,
    utils::LazyHash,
};
use typst_kit::fonts::{FontStore, system};

pub(super) struct PrototypeWorld {
    library: LazyHash<Library>,
    source: Source,
    fonts: FontStore,
}

impl PrototypeWorld {
    pub(super) fn new(text: &str) -> Self {
        let mut fonts = FontStore::new();
        for bytes in [
            include_bytes!("../../../assets/fonts/libertinus-serif-regular.otf").as_slice(),
            include_bytes!("../../../assets/fonts/libertinus-serif-bold.otf").as_slice(),
            include_bytes!("../../../assets/fonts/libertinus-serif-italic.otf").as_slice(),
            include_bytes!("../../../assets/fonts/libertinus-serif-bolditalic.otf").as_slice(),
        ] {
            fonts.extend(Font::iter(Bytes::new(bytes)).map(|font| {
                let info = font.info().clone();
                (font, info)
            }));
        }
        fonts.extend(system());
        Self {
            library: LazyHash::new(Library::default()),
            source: Source::detached(text),
            fonts,
        }
    }
}

impl World for PrototypeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<typst::text::FontBook> {
        self.fonts.book()
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.source.id() {
            Ok(Bytes::from_string(self.source.text().to_owned()))
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }
    fn today(&self, _: Option<typst::foundations::Duration>) -> Option<Datetime> {
        None
    }
}
