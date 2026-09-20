//! HID report-descriptor parsing: which collections an interface exposes, and
//! which report ids it declares on each of them.
//!
//! Why this exists: `@openmouse/protocol`'s driver registry matches devices
//! with `Client.isSupported(device)`, and every one of those predicates reads
//! `device.collections` — the WebHID view of a device's collections and the
//! report ids each declares. A browser populates it; hidapi does not. Without
//! it the app cannot use the protocol library's own registry, which is why
//! `src/native-hid/brands.ts` grew a hand-maintained copy of the registry
//! (driver list, brand labels, and a guessed Razer control collection) instead.
//! Porting this parser is what lets the app delete that copy and read the
//! protocols from the library, exactly as OpenMouse-Bridge does.
//!
//! Ported from OpenMouse-Bridge's `src/hid.rs` (`parse_report_descriptor`,
//! `CollectionInfo`, `ReportLayout` and the `GlobalState`/`CollectionNode`
//! helpers), same project, same format, so both hosts describe a device to the
//! protocol drivers identically. Keep the two in sync.

use std::collections::HashMap;

use serde::Serialize;

/// One collection of a HID interface, in the shape WebHID's `HIDCollectionInfo`
/// uses (`collections`, `inputReports`, `outputReports`, `featureReports`) so a
/// driver's `isSupported()` can read it unchanged.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionInfo {
    pub usage_page: u16,
    pub usage: u16,
    pub input_reports: Vec<ReportInfo>,
    pub output_reports: Vec<ReportInfo>,
    pub feature_reports: Vec<ReportInfo>,
    pub children: Vec<CollectionInfo>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportInfo {
    pub report_id: u8,
    pub items: Vec<ReportItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub report_size: u32,
    pub report_count: u32,
}

/// Total bits declared per report id on one interface, per direction. Used to
/// size a feature-report read (the protocol drivers hand back no length) and to
/// tell which interface a report id belongs to.
#[derive(Clone, Debug, Default)]
pub struct ReportLayout {
    feature_bits: HashMap<u8, usize>,
    input_bits: HashMap<u8, usize>,
    output_bits: HashMap<u8, usize>,
}

impl ReportLayout {
    /// Buffer size for reading `report_id` on this interface, including the
    /// report-id byte, or `None` when the interface does not declare it — the
    /// distinction a caller needs to skip an interface that cannot answer.
    pub fn feature_buffer_len(&self, report_id: u8) -> Option<usize> {
        self.feature_bits.get(&report_id).map(|bits| {
            bits.div_ceil(8)
                .saturating_add(1)
                .clamp(2, MAX_REPORT_BYTES)
        })
    }

    /// True when this interface declares `report_id` as an output or feature
    /// report — i.e. a command for it could be sent here.
    pub fn declares(&self, report_id: u8, feature: bool) -> bool {
        if feature {
            self.feature_bits.contains_key(&report_id)
        } else {
            self.output_bits.contains_key(&report_id)
        }
    }
}

/// hidapi's own descriptor ceiling; larger reports are rejected as malformed
/// rather than truncated, matching the Bridge's bound.
const MAX_REPORT_BYTES: usize = 4096;

#[derive(Clone, Default)]
struct GlobalState {
    usage_page: u16,
    report_size: u32,
    report_count: u32,
    report_id: u8,
}

struct CollectionNode {
    info: CollectionInfo,
    parent: Option<usize>,
}

fn build_collection(index: usize, nodes: &[CollectionNode]) -> CollectionInfo {
    let mut info = nodes[index].info.clone();
    for (child_index, node) in nodes.iter().enumerate() {
        if node.parent == Some(index) {
            info.children.push(build_collection(child_index, nodes));
        }
    }
    info
}

fn append_report(reports: &mut Vec<ReportInfo>, report_id: u8, item: ReportItem) {
    if let Some(existing) = reports.iter_mut().find(|report| report.report_id == report_id) {
        existing.items.push(item);
        return;
    }
    reports.push(ReportInfo {
        report_id,
        items: vec![item],
    });
}

fn item_value(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .enumerate()
        .fold(0u32, |value, (shift, byte)| {
            value | (u32::from(*byte) << (shift * 8))
        })
}

/// Parses a HID report descriptor into its top-level collections and the report
/// ids they declare.
pub fn parse_report_descriptor(bytes: &[u8]) -> Result<(Vec<CollectionInfo>, ReportLayout), String> {
    let mut globals = GlobalState::default();
    let mut global_stack = Vec::new();
    let mut usages = Vec::<u32>::new();
    let mut nodes = Vec::<CollectionNode>::new();
    let mut collections = Vec::<usize>::new();
    let mut roots = Vec::<usize>::new();
    let mut layout = ReportLayout::default();
    let mut offset = 0;

    while offset < bytes.len() {
        let prefix = bytes[offset];
        offset += 1;
        if prefix == 0xfe {
            // Long item: 0xfe, size, tag, then that many payload bytes.
            if offset + 2 > bytes.len() {
                return Err("truncated long HID item".into());
            }
            let size = bytes[offset] as usize;
            offset += 2;
            if offset + size > bytes.len() {
                return Err("truncated long HID item payload".into());
            }
            offset += size;
            continue;
        }

        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        if offset + size > bytes.len() {
            return Err("truncated HID item payload".into());
        }
        let data = &bytes[offset..offset + size];
        offset += size;
        let value = item_value(data);
        let item_type = (prefix >> 2) & 0x03;
        let tag = prefix >> 4;

        match (item_type, tag) {
            (1, 0) => globals.usage_page = value as u16, // Usage Page
            (1, 7) => globals.report_size = value,       // Report Size
            (1, 8) => globals.report_id = value as u8,   // Report ID
            (1, 9) => globals.report_count = value,      // Report Count
            (1, 10) => global_stack.push(globals.clone()), // Push
            (1, 11) => {
                globals = global_stack
                    .pop()
                    .ok_or_else(|| "HID descriptor popped an empty global stack".to_owned())?;
            }
            (2, 0) => usages.push(if size == 4 && value > u16::MAX as u32 {
                value
            } else {
                (u32::from(globals.usage_page) << 16) | (value & 0xffff)
            }),
            (0, 10) => {
                // Collection start.
                let usage = usages.first().copied().unwrap_or(0);
                let index = nodes.len();
                let parent = collections.last().copied();
                nodes.push(CollectionNode {
                    info: CollectionInfo {
                        usage_page: (usage >> 16) as u16,
                        usage: usage as u16,
                        ..CollectionInfo::default()
                    },
                    parent,
                });
                if parent.is_none() {
                    roots.push(index);
                }
                collections.push(index);
                usages.clear();
            }
            (0, 12) => {
                // Collection end.
                collections
                    .pop()
                    .ok_or_else(|| "HID descriptor closed an unopened collection".to_owned())?;
                usages.clear();
            }
            (0, 8 | 9 | 11) => {
                // Input (8), Output (9), Feature (11).
                if let Some(collection) = collections.last().copied() {
                    let item = ReportItem {
                        report_size: globals.report_size,
                        report_count: globals.report_count,
                    };
                    let reports = match tag {
                        8 => &mut nodes[collection].info.input_reports,
                        9 => &mut nodes[collection].info.output_reports,
                        _ => &mut nodes[collection].info.feature_reports,
                    };
                    append_report(reports, globals.report_id, item);
                    let bits = usize::try_from(globals.report_size)
                        .unwrap_or(usize::MAX)
                        .saturating_mul(usize::try_from(globals.report_count).unwrap_or(usize::MAX));
                    let totals = match tag {
                        8 => &mut layout.input_bits,
                        9 => &mut layout.output_bits,
                        _ => &mut layout.feature_bits,
                    };
                    let total = totals.entry(globals.report_id).or_default();
                    *total = total.saturating_add(bits);
                }
                usages.clear();
            }
            (0, _) => usages.clear(),
            _ => {}
        }
    }

    if !collections.is_empty() {
        return Err("HID descriptor ended inside a collection".into());
    }
    Ok((
        roots
            .into_iter()
            .map(|index| build_collection(index, &nodes))
            .collect(),
        layout,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from OpenMouse-Bridge's own test of the same descriptor.
    #[test]
    fn descriptor_becomes_collections_and_report_lengths() {
        let descriptor = [
            0x05, 0x01, // Usage Page (Generic Desktop)
            0x09, 0x02, // Usage (Mouse)
            0xa1, 0x01, // Collection (Application)
            0x85, 0x08, // Report ID (8)
            0x09, 0x01, // Usage (Pointer)
            0xa1, 0x00, // Collection (Physical)
            0x95, 0x10, // Report Count (16)
            0x75, 0x01, // Report Size (1)
            0x81, 0x02, // Input (Data,Var,Abs)
            0xc0, // End Collection (Physical)
            0x95, 0x01, // Report Count (1)
            0x75, 0x08, // Report Size (8)
            0x91, 0x02, // Output (Data,Var,Abs)
            0x85, 0x05, // Report ID (5)
            0x95, 0x40, // Report Count (64)
            0x75, 0x08, // Report Size (8)
            0xb1, 0x02, // Feature (Data,Var,Abs)
            0xc0, // End Collection (Application)
        ];
        let (collections, layout) = parse_report_descriptor(&descriptor).unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].usage_page, 0x0001);
        assert_eq!(collections[0].usage, 0x0002);
        assert_eq!(collections[0].children.len(), 1);
        // The input report is declared inside the nested physical collection,
        // the output and feature reports on the application collection itself.
        assert_eq!(collections[0].children[0].input_reports[0].report_id, 8);
        assert_eq!(
            collections[0].children[0].input_reports[0].items[0].report_count,
            16
        );
        assert_eq!(collections[0].output_reports[0].report_id, 8);
        assert_eq!(collections[0].feature_reports[0].report_id, 5);
        // 64 bytes of payload plus the report-id byte.
        assert_eq!(layout.feature_buffer_len(5), Some(65));
        assert!(layout.declares(5, true));
        assert!(layout.declares(8, false));
        // The distinction this parser exists for: an interface that does not
        // declare a report id must be recognisable as unable to answer it.
        assert_eq!(layout.feature_buffer_len(9), None);
        assert!(!layout.declares(9, true));
    }

    #[test]
    fn truncated_descriptors_are_rejected_not_guessed() {
        assert!(parse_report_descriptor(&[0x05, 0x01, 0x09]).is_err());
        assert!(parse_report_descriptor(&[0x05, 0x01, 0x09, 0x02, 0xa1, 0x01]).is_err());
    }
}
