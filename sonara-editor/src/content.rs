// SPDX-License-Identifier: MPL-2.0

use eframe::egui::{self, RichText};
use sonara_model::{
    Event, EventContentNode, EventContentRoot, NodeId, NodeRef, SamplerNode, SwitchCase, SwitchNode,
};
use uuid::Uuid;

use crate::{EditorLocale, TextKey, text};

#[derive(Debug, Clone)]
pub(crate) struct AssetOption {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EnumParameterOption {
    pub(crate) id: sonara_model::ParameterId,
    pub(crate) name: String,
    pub(crate) default_value: String,
    pub(crate) variants: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventContentMode {
    SingleAsset,
    StateSwitch,
    Unsupported,
}

#[derive(Debug, Clone)]
struct EventContentSummary {
    mode: EventContentMode,
    asset_id: Option<Uuid>,
    parameter_id: Option<sonara_model::ParameterId>,
    default_variant: Option<String>,
    cases: Vec<(String, Uuid)>,
}

pub(crate) fn draw_event_content_editor(
    ui: &mut egui::Ui,
    locale: EditorLocale,
    event: &mut Event,
    asset_options: &[AssetOption],
    enum_parameter_options: &[EnumParameterOption],
    changed: &mut bool,
) {
    ui.label(RichText::new(text(locale, TextKey::EventContent)).strong());
    let mut content_changed = false;

    if asset_options.is_empty() {
        ui.label(text(locale, TextKey::CreateEventNeedsAsset));
        return;
    }

    let summary = summarize_event_content(event);

    match summary.mode {
        EventContentMode::SingleAsset => {
            ui.label(format!(
                "{}: {}",
                text(locale, TextKey::ContentMode),
                text(locale, TextKey::SingleAsset)
            ));

            if let Some(asset_id) = summary.asset_id {
                let mut selected_asset_id = asset_id;
                egui::ComboBox::from_id_salt(("event_sampler_asset", event.id.0))
                    .selected_text(format_asset_display(asset_options, selected_asset_id))
                    .show_ui(ui, |ui| {
                        for asset in asset_options {
                            content_changed |= ui
                                .selectable_value(&mut selected_asset_id, asset.id, &asset.name)
                                .changed();
                        }
                    });

                if selected_asset_id != asset_id {
                    set_event_root_to_sampler(event, selected_asset_id);
                    content_changed = true;
                }
            }

            if enum_parameter_options.is_empty() {
                ui.label(text(locale, TextKey::NoEnumParameters));
            } else if ui
                .button(text(locale, TextKey::ConvertToStateSwitch))
                .clicked()
            {
                let parameter = &enum_parameter_options[0];
                let default_asset_id = summary.asset_id.unwrap_or(asset_options[0].id);
                set_event_root_to_switch(event, parameter, default_asset_id, None);
                content_changed = true;
            }
        }
        EventContentMode::StateSwitch => {
            ui.label(format!(
                "{}: {}",
                text(locale, TextKey::ContentMode),
                text(locale, TextKey::StateSwitch)
            ));

            if enum_parameter_options.is_empty() {
                ui.label(text(locale, TextKey::NoEnumParameters));
                if ui
                    .button(text(locale, TextKey::ConvertToSingleAsset))
                    .clicked()
                {
                    let fallback_asset_id = summary
                        .cases
                        .first()
                        .map(|(_, asset_id)| *asset_id)
                        .unwrap_or(asset_options[0].id);
                    set_event_root_to_sampler(event, fallback_asset_id);
                    *changed = true;
                }
                return;
            }

            let current_parameter_id = summary.parameter_id.unwrap_or(enum_parameter_options[0].id);
            let mut selected_parameter_id = current_parameter_id;

            ui.label(text(locale, TextKey::SwitchParameter));
            egui::ComboBox::from_id_salt(("event_switch_parameter", event.id.0))
                .selected_text(format_parameter_display(
                    enum_parameter_options,
                    selected_parameter_id,
                ))
                .show_ui(ui, |ui| {
                    for parameter in enum_parameter_options {
                        content_changed |= ui
                            .selectable_value(
                                &mut selected_parameter_id,
                                parameter.id,
                                &parameter.name,
                            )
                            .changed();
                    }
                });

            let selected_parameter = enum_parameter_options
                .iter()
                .find(|parameter| parameter.id == selected_parameter_id)
                .unwrap_or(&enum_parameter_options[0]);

            let default_asset_id = summary
                .cases
                .first()
                .map(|(_, asset_id)| *asset_id)
                .or(summary.asset_id)
                .unwrap_or(asset_options[0].id);
            let mut case_assets = selected_parameter
                .variants
                .iter()
                .map(|variant| {
                    let asset_id = summary
                        .cases
                        .iter()
                        .find(|(case_variant, _)| case_variant == variant)
                        .map(|(_, asset_id)| *asset_id)
                        .unwrap_or(default_asset_id);
                    (variant.clone(), asset_id)
                })
                .collect::<Vec<_>>();

            let mut default_variant = summary
                .default_variant
                .clone()
                .or_else(|| Some(selected_parameter.default_value.clone()))
                .unwrap_or_else(|| selected_parameter.variants[0].clone());

            ui.label(text(locale, TextKey::SwitchVariants));
            for (variant, asset_id) in &mut case_assets {
                ui.horizontal(|ui| {
                    ui.label(variant.as_str());
                    egui::ComboBox::from_id_salt((
                        "event_switch_case_asset",
                        event.id.0,
                        variant.clone(),
                    ))
                    .selected_text(format_asset_display(asset_options, *asset_id))
                    .show_ui(ui, |ui| {
                        for asset in asset_options {
                            content_changed |= ui
                                .selectable_value(asset_id, asset.id, &asset.name)
                                .changed();
                        }
                    });
                });
            }

            ui.label(text(locale, TextKey::DefaultCase));
            egui::ComboBox::from_id_salt(("event_switch_default_case", event.id.0))
                .selected_text(default_variant.as_str())
                .show_ui(ui, |ui| {
                    for variant in &selected_parameter.variants {
                        content_changed |= ui
                            .selectable_value(&mut default_variant, variant.clone(), variant)
                            .changed();
                    }
                });

            if ui
                .button(text(locale, TextKey::ConvertToSingleAsset))
                .clicked()
            {
                let fallback_asset_id = case_assets
                    .iter()
                    .find(|(variant, _)| variant == &default_variant)
                    .map(|(_, asset_id)| *asset_id)
                    .unwrap_or(default_asset_id);
                set_event_root_to_sampler(event, fallback_asset_id);
                content_changed = true;
            } else if selected_parameter_id != current_parameter_id || content_changed {
                set_event_root_to_switch(
                    event,
                    selected_parameter,
                    default_asset_id,
                    Some((&case_assets, default_variant.as_str())),
                );
                content_changed = true;
            }
        }
        EventContentMode::Unsupported => {
            ui.label(text(locale, TextKey::UnsupportedEventContent));
            if ui
                .button(text(locale, TextKey::ConvertToSingleAsset))
                .clicked()
            {
                set_event_root_to_sampler(event, asset_options[0].id);
                content_changed = true;
            }
            if !enum_parameter_options.is_empty()
                && ui
                    .button(text(locale, TextKey::ConvertToStateSwitch))
                    .clicked()
            {
                set_event_root_to_switch(
                    event,
                    &enum_parameter_options[0],
                    asset_options[0].id,
                    None,
                );
                content_changed = true;
            }
        }
    }

    *changed |= content_changed;
}

pub(crate) fn parse_variant_list(input: &str) -> Vec<String> {
    let mut variants = Vec::new();

    for value in input.split(',') {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !variants.iter().any(|variant| variant == trimmed) {
            variants.push(trimmed.to_owned());
        }
    }

    variants
}

fn summarize_event_content(event: &Event) -> EventContentSummary {
    let Some(root_node) = event
        .root
        .nodes
        .iter()
        .find(|node| node.id() == event.root.root.id)
    else {
        return EventContentSummary {
            mode: EventContentMode::Unsupported,
            asset_id: None,
            parameter_id: None,
            default_variant: None,
            cases: Vec::new(),
        };
    };

    match root_node {
        EventContentNode::Sampler(node) => EventContentSummary {
            mode: EventContentMode::SingleAsset,
            asset_id: Some(node.asset_id),
            parameter_id: None,
            default_variant: None,
            cases: Vec::new(),
        },
        EventContentNode::Switch(node) => {
            let mut cases = Vec::with_capacity(node.cases.len());
            let mut default_variant = None;

            for case in &node.cases {
                let Some(EventContentNode::Sampler(sampler)) = event
                    .root
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id() == case.child.id)
                else {
                    return EventContentSummary {
                        mode: EventContentMode::Unsupported,
                        asset_id: None,
                        parameter_id: None,
                        default_variant: None,
                        cases: Vec::new(),
                    };
                };

                if node.default_case == Some(case.child) {
                    default_variant = Some(case.variant.to_string());
                }
                cases.push((case.variant.to_string(), sampler.asset_id));
            }

            EventContentSummary {
                mode: EventContentMode::StateSwitch,
                asset_id: cases.first().map(|(_, asset_id)| *asset_id),
                parameter_id: Some(node.parameter_id),
                default_variant,
                cases,
            }
        }
        _ => EventContentSummary {
            mode: EventContentMode::Unsupported,
            asset_id: None,
            parameter_id: None,
            default_variant: None,
            cases: Vec::new(),
        },
    }
}

fn set_event_root_to_sampler(event: &mut Event, asset_id: Uuid) {
    let sampler_id = NodeId::new();
    event.root = EventContentRoot {
        root: NodeRef { id: sampler_id },
        nodes: vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id,
        })],
    };
}

fn set_event_root_to_switch(
    event: &mut Event,
    parameter: &EnumParameterOption,
    fallback_asset_id: Uuid,
    case_mapping: Option<(&[(String, Uuid)], &str)>,
) {
    let switch_id = NodeId::new();
    let mut nodes = Vec::with_capacity(parameter.variants.len() + 1);
    let mut cases = Vec::with_capacity(parameter.variants.len());
    let mut default_case = None;

    for variant in &parameter.variants {
        let sampler_id = NodeId::new();
        let asset_id = case_mapping
            .and_then(|(mapping, _)| {
                mapping
                    .iter()
                    .find(|(case_variant, _)| case_variant == variant)
                    .map(|(_, asset_id)| *asset_id)
            })
            .unwrap_or(fallback_asset_id);
        let child = NodeRef { id: sampler_id };

        if case_mapping
            .map(|(_, default_variant)| default_variant == variant.as_str())
            .unwrap_or_else(|| parameter.default_value == *variant)
        {
            default_case = Some(child);
        }

        cases.push(SwitchCase {
            variant: variant.clone().into(),
            child,
        });
        nodes.push(EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id,
        }));
    }

    nodes.insert(
        0,
        EventContentNode::Switch(SwitchNode {
            id: switch_id,
            parameter_id: parameter.id,
            cases,
            default_case,
        }),
    );
    event.root = EventContentRoot {
        root: NodeRef { id: switch_id },
        nodes,
    };
    if !event.default_parameters.contains(&parameter.id) {
        event.default_parameters.push(parameter.id);
    }
}

fn format_asset_display(asset_options: &[AssetOption], asset_id: Uuid) -> String {
    asset_options
        .iter()
        .find(|asset| asset.id == asset_id)
        .map(|asset| asset.name.clone())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn format_parameter_display(
    parameter_options: &[EnumParameterOption],
    parameter_id: sonara_model::ParameterId,
) -> String {
    parameter_options
        .iter()
        .find(|parameter| parameter.id == parameter_id)
        .map(|parameter| parameter.name.clone())
        .unwrap_or_else(|| "unknown".to_owned())
}
