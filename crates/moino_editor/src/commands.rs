//! Public actions used by every formatting entry point.
use gpui_kit::actions;
actions!(
    inline,
    [
        /// Toggles strong emphasis.
        Bold,
        /// Toggles emphasis.
        Italic,
        /// Toggles inline code.
        Code,
        /// Toggles strikethrough.
        Strike,
        /// Toggles highlighted text.
        Highlight,
        /// Toggles subscript.
        Subscript,
        /// Toggles superscript.
        Superscript,
    ]
);
actions!(
    block,
    [
        /// Converts to a paragraph.
        Paragraph,
        /// Toggles a level-one heading.
        Heading1,
        /// Toggles a level-two heading.
        Heading2,
        /// Toggles a level-three heading.
        Heading3,
        /// Toggles a level-four heading.
        Heading4,
        /// Toggles a level-five heading.
        Heading5,
        /// Toggles a level-six heading.
        Heading6,
        /// Toggles a blockquote.
        Quote,
        /// Toggles a bullet list.
        BulletList,
        /// Toggles an ordered list.
        OrderedList,
        /// Toggles a task list.
        TaskList,
    ]
);
actions!(
    edit,
    [
        /// Removes formatting from the selected source.
        ClearFormatting,
    ]
);
