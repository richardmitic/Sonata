// Sonata
// Copyright (c) 2019 The Sonata Project Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `meta` module defines basic metadata elements, and management structures.

use std::cell::{Ref, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroU32;
use std::ops::Deref;

use crate::errors::Result;
use crate::io::MediaSourceStream;

/// Limit defines how a `Format` or `Codec` should handle resource allocation when the amount of
/// that resource to be allocated is dictated by the untrusted stream. Limits are used to prevent
/// denial-of-service attacks whereby the stream requests the `Format` or `Codec` to allocate large
/// amounts of a resource, usually memory. A limit will place an upper-bound on this allocation at
/// the risk of breaking potentially valid streams.
///
/// All limits can be defaulted to a reasonable value specific to the situation. These defaults will
/// generally not break any normal stream.
#[derive(Copy, Clone)]
pub enum Limit {
    /// Do not impose any limit.
    None,
    /// Use the (reasonable) default specified by the `Format` or `Codec`.
    Default,
    /// Specify the upper limit of the resource. Units are use-case specific.
    Maximum(usize),
}

impl Limit {
    /// Gets the numeric limit of the limit, or default value. If there is no limit, None is
    /// returned.
    pub fn limit_or_default(&self, default: usize) -> Option<usize> {
        match self {
            Limit::None => None,
            Limit::Default => Some(default),
            Limit::Maximum(max) => Some(*max),
        }
    }
}

/// `MetadataOptions` is a common set of options that all metadata readers use.
#[derive(Copy, Clone)]
pub struct MetadataOptions {
    /// The maximum size limit in bytes that a tag may occupy in memory once decoded. Tags exceeding
    /// this limit will be skipped by the demuxer. Take note that tags in-memory are stored as UTF-8
    /// and therefore may occupy more than one byte per character.
    pub limit_metadata_bytes: Limit,

    /// The maximum size limit in bytes that a visual (picture) may occupy.
    pub limit_visual_bytes: Limit,
}

impl Default for MetadataOptions {
    fn default() -> Self {
        MetadataOptions {
            limit_metadata_bytes: Limit::Default,
            limit_visual_bytes: Limit::Default,
        }
    }
}

/// `StandardVisualKey` is an enumation providing standardized keys for common visual dispositions.
/// A demuxer may assign a `StandardVisualKey` to a `Visual` if the disposition of the attached
/// visual is known and can be mapped to a standard key.
///
/// The visual types listed here are derived from, though do not entirely cover, the ID3v2 APIC
/// frame specification.
#[derive(Copy, Clone, Debug)]
pub enum StandardVisualKey {
    FileIcon,
    OtherIcon,
    FrontCover,
    BackCover,
    Leaflet,
    Media,
    LeadArtistPerformerSoloist,
    ArtistPerformer,
    Conductor,
    BandOrchestra,
    Composer,
    Lyricist,
    RecordingLocation,
    RecordingSession,
    Performance,
    ScreenCapture,
    Illustration,
    BandArtistLogo,
    PublisherStudioLogo,
}

/// `StandardTagKey` is an enumation providing standardized keys for common tag types.
/// A tag reader may assign a `StandardTagKey` to a `Tag` if the tag's key is generally
/// accepted to map to a specific usage.
#[derive(Copy, Clone, Debug)]
pub enum StandardTagKey {
    AcoustidFingerprint,
    AcoustidId,
    Album,
    AlbumArtist,
    Arranger,
    Artist,
    Bpm,
    Comment,
    Compilation,
    Composer,
    Conductor,
    ContentGroup,
    Copyright,
    Date,
    Description,
    DiscNumber,
    DiscSubtitle,
    DiscTotal,
    EncodedBy,
    Encoder,
    EncoderSettings,
    EncodingDate,
    Engineer,
    Ensemble,
    Genre,
    IdentBarcode,
    IdentCatalogNumber,
    IdentEanUpn,
    IdentIsrc,
    IdentPn,
    IdentPodcast,
    IdentUpc,
    Label,
    Language,
    License,
    Lyricist,
    Lyrics,
    MediaFormat,
    MixDj,
    MixEngineer,
    Mood,
    MovementName,
    MovementNumber,
    MusicBrainzAlbumArtistId,
    MusicBrainzAlbumId,
    MusicBrainzArtistId,
    MusicBrainzDiscId,
    MusicBrainzGenreId,
    MusicBrainzLabelId,
    MusicBrainzOriginalAlbumId,
    MusicBrainzOriginalArtistId,
    MusicBrainzRecordingId,
    MusicBrainzReleaseGroupId,
    MusicBrainzReleaseTrackId,
    MusicBrainzTrackId,
    MusicBrainzWorkId,
    Opus,
    OriginalAlbum,
    OriginalArtist,
    OriginalDate,
    OriginalFile,
    OriginalWriter,
    Part,
    PartTotal,
    Performer,
    Podcast,
    PodcastCategory,
    PodcastDescription,
    PodcastKeywords,
    Producer,
    Rating,
    ReleaseCountry,
    ReleaseDate,
    Remixer,
    ReplayGainAlbumGain,
    ReplayGainAlbumPeak,
    ReplayGainTrackGain,
    ReplayGainTrackPeak,
    Script,
    SortAlbum,
    SortAlbumArtist,
    SortArtist,
    SortComposer,
    SortTrackTitle,
    TaggingDate,
    TrackNumber,
    TrackSubtitle,
    TrackTitle,
    TrackTotal,
    Url,
    UrlArtist,
    UrlCopyright,
    UrlInternetRadio,
    UrlLabel,
    UrlOfficial,
    UrlPayment,
    UrlPodcast,
    UrlPurchase,
    UrlSource,
    Version,
    Writer,
}

/// A `Tag` encapsulates a key-value pair of metadata.
pub struct Tag {
    /// If the `Tag`'s key string is commonly associated with a typical type, meaning, or purpose,
    /// then if recognized a `StandardTagKey` will be assigned to this `Tag`.
    ///
    /// This is a best effort guess since not all metadata formats have a well defined or specified
    /// mapping. However, it is recommended that user's use `std_key` over `key` if provided.
    pub std_key: Option<StandardTagKey>,
    /// A key string indicating the type, meaning, or purpose of the `Tag`s value.
    pub key: String,
    /// The value of the `Tag`.
    pub value: String,
}

impl Tag {
    /// Create a new `Tag`.
    pub fn new(std_key: Option<StandardTagKey>, key: &str, value: &str) -> Tag {
        Tag {
            std_key,
            key: key.to_string(),
            value: value.to_string()
        }
    }

    /// Returns true if the `Tag`'s key string was recognized and a `StandardTagKey` was assigned,
    /// otherwise false is returned.
    pub fn is_known(&self) -> bool {
        self.std_key.is_some()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.std_key {
            Some(ref std_key) =>
                write!(
                    f,
                    "{{ std_key={:?}, key=\"{}\", value=\"{}\" }}",
                    std_key,
                    self.key,
                    self.value
                ),
            None => write!(f, "{{ key=\"{}\", value=\"{}\" }}", self.key, self.value),
        }
    }
}

/// A 2 dimensional (width and height) size type.
#[derive(Copy, Clone)]
pub struct Size {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

/// `ColorMode` indicates how the color of a pixel is encoded in a `Visual`.
#[derive(Copy, Clone)]
pub enum ColorMode {
    /// Each pixel in the `Visual` stores its own color information.
    Discrete,
    /// Each pixel in the `Visual` stores an index into a color palette containing the color
    /// information. The value stored by this variant indicates the number of colors in the color
    /// palette.
    Indexed(NonZeroU32),
}

/// A `Visual` is any 2 dimensional graphic.
pub struct Visual {
    /// The Media Type (MIME Type) used to encode the `Visual`.
    pub media_type: String,
    /// The dimensions of the `Visual`.
    ///
    /// Note: This value may not be accurate as it comes from metadata, not the embedded graphic
    /// itself. Consider it only a hint.
    pub dimensions: Option<Size>,
    /// The number of bits-per-pixel (aka bit-depth) of the unencoded image.
    ///
    /// Note: This value may not be accurate as it comes from metadata, not the embedded graphic
    /// itself. Consider it only a hint.
    pub bits_per_pixel: Option<NonZeroU32>,
    /// The color mode of the `Visual`.
    ///
    /// Note: This value may not be accurate as it comes from metadata, not the embedded graphic
    /// itself. Consider it only a hint.
    pub color_mode: Option<ColorMode>,
    /// The usage and/or content of the `Visual`.
    pub usage: Option<StandardVisualKey>,
    /// Any tags associated with the `Visual`.
    pub tags: Vec<Tag>,
    /// The data of the `Visual`, encoded as per `media_type`.
    pub data: Box<[u8]>,
}

/// `VendorData` is any binary metadata that is proprietary to a certain application or vendor.
pub struct VendorData {
    /// A text representation of the vendor's application identifier.
    pub ident: String,
    /// The vendor data.
    pub data: Box<[u8]>,
}

/// `Metadata` is a container for a single discrete revision of metadata information.
#[derive(Default)]
pub struct Metadata {
    tags: Vec<Tag>,
    visuals: Vec<Visual>,
    vendor_data: Vec<VendorData>,
}

impl Metadata {
    /// Gets an immutable slice to the `Tag`s in this revision.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Gets an immutable slice to the `Visual`s in this revision.
    pub fn visuals(&self) -> &[Visual] {
        &self.visuals
    }

    /// Gets an immutable slice to the `VendorData` in this revision.
    pub fn vendor_data(&self) -> &[VendorData] {
        &self.vendor_data
    }
}

/// `MetadataBuilder` is the builder for `Metadata` revisions.
pub struct MetadataBuilder {
    metadata: Metadata,
}

impl MetadataBuilder {
    /// Instantiate a new `MetadataBuilder`.
    pub fn new() -> Self {
        MetadataBuilder {
            metadata: Default::default(),
        }
    }

    /// Add a `Tag` to the metadata.
    pub fn add_tag(&mut self, tag: Tag) -> &mut Self {
        self.metadata.tags.push(tag);
        self
    }

    /// Add a `Visual` to the metadata.
    pub fn add_visual(&mut self, visual: Visual) -> &mut Self {
        self.metadata.visuals.push(visual);
        self
    }

    /// Add `VendorData` to the metadata.
    pub fn add_vendor_data(&mut self, vendor_data: VendorData) -> &mut Self {
        self.metadata.vendor_data.push(vendor_data);
        self
    }

    /// Yield the constructed `Metadata` revision.
    pub fn metadata(self) -> Metadata {
        self.metadata
    }
}

/// An immutable reference to a `Metadata` revision.
pub struct MetadataRef<'a> {
    guard: Ref<'a, VecDeque<Metadata>>,
}

impl<'a> Deref for MetadataRef<'a> {
    type Target = Metadata;

    fn deref(&self) -> &Metadata {
        // MetadataQueue should never instantiate a MetadataRef if there is no Metadata struct
        // enqueued.
        &self.guard.front().unwrap()
    }
}

/// `MetadataQueue` is a container for time-ordered `Metadata` revisions.
#[derive(Default)]
pub struct MetadataQueue {
    queue: RefCell<VecDeque<Metadata>>,
}

impl MetadataQueue {
    /// Returns `true` if the current metadata revision is the newest, `false` otherwise.
    pub fn is_latest(&self) -> bool {
        self.queue.borrow().len() < 2
    }

    /// Gets an immutable reference to the current, and therefore oldest, revision of the metadata.
    pub fn current(&self) -> Option<MetadataRef> {
        let queue = self.queue.borrow();

        if queue.len() > 0 {
            Some(MetadataRef { guard: queue })
        }
        else {
            None
        }
    }

    /// If there are newer `Metadata` revisions, advances the `MetadataQueue` by discarding the
    /// current revision and replacing it with the next revision, returning the discarded
    /// `Metadata`. When there are no newer revisions, `None` is returned. As such, `pop` will never
    /// completely empty the queue.
    pub fn pop(&self) -> Option<Metadata> {
        let mut queue = self.queue.borrow_mut();

        if queue.len() > 1 {
            queue.pop_front()
        }
        else {
            None
        }
    }

    /// Pushes a new `Metadata` revision onto the queue.
    pub fn push(&mut self, rev: Metadata) {
        self.queue.borrow_mut().push_back(rev);
    }
}

pub trait MetadataReader {
    /// Instantiates the `MetadataReader` with the provided `MetadataOptions`.
    fn new(options: &MetadataOptions) -> Self
    where
        Self: Sized;

    /// Read all metadata and return it if successful.
    fn read_all(&mut self, reader: &mut MediaSourceStream) -> Result<Metadata>;
}