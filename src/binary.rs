use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use object::{
    Architecture, Endian, File, Object, ObjectSection, macho, read::macho::LoadCommandVariant,
};
use tracing::{debug, warn};

use crate::OSVersion;

/// Extracted information about a binary.
#[derive(Debug)]
pub struct Binary {
    /// The path to the binary.
    pub path: PathBuf,
    /// The architecture of the binary.
    pub arch: Architecture,
    /// `LC_ID_DYLIB`.
    pub name: Option<Vec<u8>>,
    /// Usually there's only one here, but we expect to see two of these in a
    /// zippered binary (a binary that supports both macOS and Mac Catalyst).
    pub versions: Vec<BuildVersion>,
    /// Embedded `__TEXT,__info_plist` contents, if any.
    ///
    /// See the `embed_plist` crate for how to insert this.
    pub info_plist_data: Option<Vec<u8>>,
    /// Embedded `__TEXT,__entitlements` contents, if any.
    ///
    /// See the `embed_entitlements` crate for how to insert this.
    pub entitlements_data: Option<Vec<u8>>,
    /// Whether the binary is already (likely ad-hoc) signed.
    pub signed: bool,
    // TODO: Support manganis __ASSET__?
    // Potentially also special assets like:
    // - asset catalogs (`actool --version --output-format xml1`)
    // - Interface builder (`ictool --version --output-format xml1`)
    // And possibly plist/entitlement information from this too? Though also
    // maybe nice to keep that separate.
}

impl Binary {
    pub fn parse(path: &Path) -> Result<Self> {
        let file = std::fs::read(path).context("failed reading")?;

        let file = File::parse(&*file).context("failed parsing")?;

        let (endianness, cputype, filetype) = match &file {
            File::MachO32(m) => (
                m.endian(),
                m.macho_header().cputype.get(m.endian()),
                m.macho_header().filetype.get(m.endian()),
            ),
            File::MachO64(m) => (
                m.endian(),
                m.macho_header().cputype.get(m.endian()),
                m.macho_header().filetype.get(m.endian()),
            ),
            _ => bail!("not a Mach-O file: {file:?}"),
        };

        if filetype != macho::MH_EXECUTE {
            warn!("unsupported file type {filetype:02x}");
        }

        let load_commands = match &file {
            File::MachO32(m) => m.macho_load_commands(),
            File::MachO64(m) => m.macho_load_commands(),
            _ => bail!("not a Mach-O file"),
        };

        debug!("vtool -show-build {path:?}");
        let mut versions = Vec::new();
        let mut signed = false;
        let mut name = None;
        for cmd in load_commands.context("failed reading load command")? {
            let cmd = cmd.context("failed reading load command")?;
            if let Ok(variant) = cmd.variant() {
                if let Some(v) = BuildVersion::from_load_command(variant, cputype, endianness) {
                    versions.push(v);
                }
                if let LoadCommandVariant::IdDylib(dylib_cmd) = variant {
                    let s = cmd.string(endianness, dylib_cmd.dylib.name);
                    name = Some(s.context("failed reading LC_ID_DYLIB")?.to_vec());
                }
            }
            if cmd.cmd() == macho::LC_CODE_SIGNATURE {
                signed = true;
            }
        }

        match versions.len() {
            0 => warn!("binary had no version information"),
            1 => {}
            _ => warn!("zippered binaries aren't yet properly supported"),
        }

        debug!("segedit {path:?} -extract __TEXT __info_plist /dev/stdout");
        let info_plist_data = if let Some(section) = file.section_by_name("__info_plist") {
            let segment_name = section
                .segment_name_bytes()
                .context("failed reading segment name")?;
            if segment_name != Some(b"__TEXT") {
                warn!("__info_plist was not in __TEXT segment");
            }
            let data = section.data().context("failed reading section contents")?;
            Some(data.to_vec())
        } else {
            None
        };

        debug!("segedit {path:?} -extract __TEXT __entitlements /dev/stdout");
        let entitlements_data = if let Some(section) = file.section_by_name("__entitlements") {
            let segment_name = section
                .segment_name_bytes()
                .context("failed reading segment name")?;
            if segment_name != Some(b"__TEXT") {
                warn!("__entitlements was not in __TEXT segment");
            }
            let data = section.data().context("failed reading section contents")?;
            Some(data.to_vec())
        } else {
            None
        };

        // TODO: Read `__ASSETS__` that manganis inserts?

        Ok(Self {
            path: path.to_owned(),
            arch: file.architecture(),
            name,
            versions,
            info_plist_data,
            entitlements_data,
            signed,
        })
    }

    fn version(&self) -> BuildVersion {
        self.versions.first().copied().unwrap_or_default()
    }

    pub fn platform(&self) -> Platform {
        self.version().platform
    }

    pub fn minos(&self) -> OSVersion {
        self.version().minos
    }

    pub fn sdk(&self) -> OSVersion {
        self.version().sdk
    }

    pub(crate) fn needs_info_plist(&self) -> bool {
        // TODO: Probably? Needs to be tested
        !matches!(
            self.version().platform,
            Platform::MACOS
                | Platform::IOSSIMULATOR
                | Platform::TVOSSIMULATOR
                | Platform::WATCHOSSIMULATOR
                | Platform::VISIONOSSIMULATOR
        )
    }
}

/// Simplified LC_BUILD_VERSION.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
struct BuildVersion {
    platform: Platform,
    minos: OSVersion,
    sdk: OSVersion,
}

impl BuildVersion {
    /// Checks here are the same as the ones the loader does:
    /// <https://github.com/apple-oss-distributions/dyld/blob/dyld-1340/mach_o/Header.cpp#L1113-L1176>
    fn from_load_command<E: Endian>(
        variant: LoadCommandVariant<'_, E>,
        cputype: u32,
        endianness: E,
    ) -> Option<Self> {
        match variant {
            LoadCommandVariant::BuildVersion(version) => Some(BuildVersion {
                platform: Platform(version.platform.get(endianness)),
                minos: OSVersion::from_packed(version.minos.get(endianness)),
                sdk: OSVersion::from_packed(version.sdk.get(endianness)),
            }),
            LoadCommandVariant::VersionMin(version) => Some(BuildVersion {
                platform: match version.cmd.get(endianness) {
                    macho::LC_VERSION_MIN_MACOSX => Platform::MACOS,
                    macho::LC_VERSION_MIN_IPHONEOS => {
                        if matches!(cputype, macho::CPU_TYPE_X86_64 | macho::CPU_TYPE_X86) {
                            Platform::IOSSIMULATOR // old sim binary
                        } else {
                            Platform::IOS
                        }
                    }
                    macho::LC_VERSION_MIN_TVOS => {
                        if cputype == macho::CPU_TYPE_X86_64 {
                            Platform::TVOSSIMULATOR // old sim binary
                        } else {
                            Platform::TVOS
                        }
                    }
                    macho::LC_VERSION_MIN_WATCHOS => {
                        if matches!(cputype, macho::CPU_TYPE_X86_64 | macho::CPU_TYPE_X86) {
                            Platform::WATCHOSSIMULATOR // old sim binary
                        } else {
                            Platform::WATCHOS
                        }
                    }
                    _ => unreachable!(),
                },
                minos: OSVersion::from_packed(version.version.get(endianness)),
                sdk: OSVersion::from_packed(version.sdk.get(endianness)),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Platform(u32);

impl Platform {
    pub const MACOS: Self = Self(macho::PLATFORM_MACOS);
    pub const IOS: Self = Self(macho::PLATFORM_IOS);
    pub const TVOS: Self = Self(macho::PLATFORM_TVOS);
    pub const WATCHOS: Self = Self(macho::PLATFORM_WATCHOS);
    pub const BRIDGEOS: Self = Self(macho::PLATFORM_BRIDGEOS);
    pub const MACCATALYST: Self = Self(macho::PLATFORM_MACCATALYST);
    pub const IOSSIMULATOR: Self = Self(macho::PLATFORM_IOSSIMULATOR);
    pub const TVOSSIMULATOR: Self = Self(macho::PLATFORM_TVOSSIMULATOR);
    pub const WATCHOSSIMULATOR: Self = Self(macho::PLATFORM_WATCHOSSIMULATOR);
    pub const DRIVERKIT: Self = Self(macho::PLATFORM_DRIVERKIT);
    pub const VISIONOS: Self = Self(macho::PLATFORM_XROS);
    pub const VISIONOSSIMULATOR: Self = Self(macho::PLATFORM_XROSSIMULATOR);

    pub fn is_simulator(self) -> bool {
        matches!(
            self,
            Self::IOSSIMULATOR
                | Self::TVOSSIMULATOR
                | Self::WATCHOSSIMULATOR
                | Self::VISIONOSSIMULATOR
        )
    }
}
