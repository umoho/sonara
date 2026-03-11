use eframe::egui::{self, Align, Color32, Layout, RichText, TextEdit};
use sonara_build::collect_event_asset_ids;
use sonara_model::{EventKind, Parameter, ParameterScope, SpatialMode};

use crate::{
    EditorLocale, EditorState, LogLevel, SelectedItem, TextKey, TextTemplate,
    content::draw_event_content_editor, format_event_kind_display, format_parameter_scope_display,
    format_spatial_mode_display,
};

impl EditorState {
    pub(crate) fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("project_toolbar")
            .resizable(false)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    let toggle_project_panel = self.tx(TextKey::ToggleProjectPanel);
                    let toggle_inspector_panel = self.tx(TextKey::ToggleInspectorPanel);
                    let toggle_tool_strip = self.tx(TextKey::ToggleToolStrip);
                    let toggle_status_bar = self.tx(TextKey::ToggleStatusBar);
                    let window_open_project = self.tx(TextKey::WindowOpenProject);
                    let window_export_bank = self.tx(TextKey::WindowExportBank);
                    let window_bank_events = self.tx(TextKey::WindowBankEvents);
                    let window_diagnostics = self.tx(TextKey::WindowDiagnostics);
                    let window_log = self.tx(TextKey::WindowLog);

                    ui.menu_button(self.tx(TextKey::MenuFile), |ui| {
                        if ui.button(self.tx(TextKey::OpenProject)).clicked() {
                            self.show_open_project_window = true;
                            ui.close();
                        }
                        if ui.button(self.tx(TextKey::SaveProject)).clicked() {
                            self.save_project();
                            ui.close();
                        }
                        if ui.button(self.tx(TextKey::WindowExportBank)).clicked() {
                            self.show_export_bank_window = true;
                            ui.close();
                        }
                    });

                    ui.menu_button(self.tx(TextKey::MenuView), |ui| {
                        ui.checkbox(&mut self.show_project_panel, toggle_project_panel);
                        ui.checkbox(&mut self.show_inspector_panel, toggle_inspector_panel);
                        ui.checkbox(&mut self.show_tool_strip, toggle_tool_strip);
                        ui.checkbox(&mut self.show_status_bar, toggle_status_bar);
                    });

                    ui.menu_button(self.tx(TextKey::MenuWindow), |ui| {
                        ui.checkbox(&mut self.show_open_project_window, window_open_project);
                        ui.checkbox(&mut self.show_export_bank_window, window_export_bank);
                        ui.checkbox(&mut self.show_bank_events_window, window_bank_events);
                        ui.checkbox(&mut self.show_diagnostics_window, window_diagnostics);
                        ui.checkbox(&mut self.show_log_window, window_log);
                    });

                    ui.menu_button(self.tx(TextKey::MenuHelp), |ui| {
                        if ui.button(self.tx(TextKey::WindowAbout)).clicked() {
                            self.show_about_window = true;
                            ui.close();
                        }
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("editor_locale")
                            .selected_text(self.locale.display_name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.locale,
                                    EditorLocale::ZhCn,
                                    EditorLocale::ZhCn.display_name(),
                                );
                                ui.selectable_value(
                                    &mut self.locale,
                                    EditorLocale::EnUs,
                                    EditorLocale::EnUs.display_name(),
                                );
                            });
                        ui.label(self.tx(TextKey::Language));
                    });
                });
            });
    }

    pub(crate) fn draw_status_bar(&mut self, ctx: &egui::Context) {
        if !self.show_status_bar {
            return;
        }

        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(self.tx(TextKey::Ready));
                    ui.separator();
                    let project_label = self
                        .loaded_project
                        .as_ref()
                        .map(|project| project.name.to_string())
                        .unwrap_or_else(|| self.tx(TextKey::NoProjectLoadedShort).to_owned());
                    ui.label(format!(
                        "{}: {}",
                        self.tx(TextKey::CurrentProject),
                        project_label
                    ));
                    ui.separator();
                    let bank_label = self
                        .selected_bank_name
                        .clone()
                        .unwrap_or_else(|| "-".to_owned());
                    ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank_label));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dirty_label = if self.has_unsaved_changes {
                            self.tx(TextKey::UnsavedChanges)
                        } else {
                            self.tx(TextKey::SavedProjectStatus)
                        };
                        let color = if self.has_unsaved_changes {
                            Color32::YELLOW
                        } else {
                            Color32::LIGHT_GREEN
                        };
                        ui.label(RichText::new(dirty_label).color(color));
                    });
                });
            });
    }

    pub(crate) fn draw_tool_strip(&mut self, ctx: &egui::Context) {
        if !self.show_tool_strip {
            return;
        }

        egui::TopBottomPanel::bottom("tool_strip")
            .resizable(false)
            .exact_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(self.tx(TextKey::ToolStrip));
                    ui.separator();
                    let bank_label = self
                        .selected_bank_name
                        .clone()
                        .unwrap_or_else(|| "-".to_owned());
                    ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank_label));
                    ui.separator();
                    let validation_label = if self.validation_report.can_export() {
                        self.tx(TextKey::ValidationReady)
                    } else {
                        self.tx(TextKey::ValidationBlocked)
                    };
                    let validation_color = if self.validation_report.can_export() {
                        Color32::LIGHT_GREEN
                    } else {
                        Color32::LIGHT_RED
                    };
                    ui.label(RichText::new(validation_label).color(validation_color));
                    ui.separator();
                    ui.label(self.tx(TextKey::PreviewUnavailable));
                });
            });
    }

    pub(crate) fn draw_left_panel(&mut self, ctx: &egui::Context) {
        if !self.show_project_panel {
            return;
        }

        egui::SidePanel::left("project_panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading(self.tx(TextKey::ProjectPanel));
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label(self.tx(TextKey::NoProjectLoaded));
                    return;
                };

                ui.label(format!("{}: {}", self.tx(TextKey::Name), project.name));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Assets),
                    project.assets.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Events),
                    project.events.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Parameters),
                    project.parameters.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Banks),
                    project.banks.len()
                ));

                ui.separator();
                ui.label(RichText::new(self.tx(TextKey::BankList)).strong());

                let bank_names: Vec<String> = project
                    .banks
                    .iter()
                    .map(|bank| bank.name.to_string())
                    .collect();
                let mut clicked_bank_name = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for bank_name in &bank_names {
                        let selected =
                            self.selected_bank_name.as_deref() == Some(bank_name.as_str());
                        if ui.selectable_label(selected, bank_name.as_str()).clicked() {
                            clicked_bank_name = Some(bank_name.clone());
                        }
                    }
                });

                if let Some(bank_name) = clicked_bank_name {
                    self.select_bank(&bank_name);
                }
            });
    }

    pub(crate) fn draw_center_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                ui.add_space(40.0);
                ui.heading(self.tx(TextKey::WelcomeBack));
                ui.label(RichText::new(self.tx(TextKey::WelcomeTitle)).size(28.0));
                ui.label(self.tx(TextKey::WelcomeSubtitle));

                ui.add_space(16.0);
                ui.group(|ui| {
                    ui.set_max_width(520.0);
                    ui.label(RichText::new(self.tx(TextKey::QuickActions)).strong());
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(self.tx(TextKey::OpenProject)).clicked() {
                            self.show_open_project_window = true;
                        }
                        if ui
                            .add_enabled(
                                self.loaded_project.is_some(),
                                egui::Button::new(self.tx(TextKey::ContinueProject)),
                            )
                            .clicked()
                        {
                            self.show_bank_events_window = true;
                        }
                        if ui
                            .add_enabled(
                                self.loaded_project.is_some(),
                                egui::Button::new(self.tx(TextKey::WindowExportBank)),
                            )
                            .clicked()
                        {
                            self.show_export_bank_window = true;
                        }
                        if ui
                            .add_enabled(
                                self.loaded_project.is_some(),
                                egui::Button::new(self.tx(TextKey::WindowDiagnostics)),
                            )
                            .clicked()
                        {
                            self.show_diagnostics_window = true;
                        }
                        if ui.button(self.tx(TextKey::WindowLog)).clicked() {
                            self.show_log_window = true;
                        }
                    });
                });

                ui.add_space(20.0);
                ui.group(|ui| {
                    ui.set_max_width(520.0);
                    let project_label = self
                        .loaded_project
                        .as_ref()
                        .map(|project| project.name.to_string())
                        .unwrap_or_else(|| "-".to_owned());
                    let bank_label = self
                        .selected_bank_name
                        .clone()
                        .unwrap_or_else(|| "-".to_owned());
                    ui.label(format!(
                        "{}: {}",
                        self.tx(TextKey::CurrentProject),
                        project_label
                    ));
                    ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank_label));
                    if !self.status_message.is_empty() {
                        ui.label(RichText::new(&self.status_message).color(Color32::LIGHT_BLUE));
                    }
                });

                ui.add_space(20.0);
                ui.group(|ui| {
                    ui.set_max_width(520.0);
                    ui.label(RichText::new(self.tx(TextKey::RecentProjects)).strong());
                    if self.recent_projects.is_empty() {
                        ui.label(self.tx(TextKey::NoRecentProjects));
                    } else {
                        let recent_projects = self.recent_projects.clone();
                        for path in recent_projects {
                            if ui.button(path.as_str()).clicked() {
                                self.project_path = path;
                                self.load_project();
                            }
                        }
                    }
                });
            });
        });
    }

    pub(crate) fn draw_right_panel(&mut self, ctx: &egui::Context) {
        if !self.show_inspector_panel {
            return;
        }

        egui::SidePanel::right("inspector_panel")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading(self.tx(TextKey::Inspector));
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label(self.tx(TextKey::NoProjectLoadedShort));
                    return;
                };

                let Some(bank) = self.selected_bank(project) else {
                    ui.label(self.tx(TextKey::NoBankSelected));
                    return;
                };

                ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank.name));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::EventCount),
                    bank.events.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::BusCount),
                    bank.buses.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::SnapshotCount),
                    bank.snapshots.len()
                ));
                ui.separator();
                self.draw_selected_item_inspector(ui);
            });
    }

    pub(crate) fn draw_validation_report(&self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new(self.tx(TextKey::Validation)).strong());

            let status_label = if self.validation_report.can_export() {
                self.tx(TextKey::ValidationReady)
            } else {
                self.tx(TextKey::ValidationBlocked)
            };
            let status_color = if self.validation_report.can_export() {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_RED
            };

            ui.label(RichText::new(status_label).color(status_color));

            if self.validation_report.issues.is_empty() {
                ui.label(self.tx(TextKey::NoValidationIssues));
            } else {
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::ValidationIssueCount),
                    self.validation_report.issues.len()
                ));
                for issue in &self.validation_report.issues {
                    ui.colored_label(Color32::LIGHT_RED, issue);
                }
            }

            if let Some(asset_count) = self.validation_report.asset_count {
                ui.label(format!("{}: {}", self.tx(TextKey::AssetCount), asset_count));
            }
            if let Some(resident_media_count) = self.validation_report.resident_media_count {
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::ResidentMediaCount),
                    resident_media_count
                ));
            }
            if let Some(streaming_media_count) = self.validation_report.streaming_media_count {
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::StreamingMediaCount),
                    streaming_media_count
                ));
            }
        });
    }

    pub(crate) fn draw_selected_item_inspector(&mut self, ui: &mut egui::Ui) {
        if self.loaded_project.is_none() {
            ui.label(self.tx(TextKey::NoProjectLoadedShort));
            return;
        }

        let asset_options = self.asset_options();
        let enum_parameter_options = self.enum_parameter_options();
        let node_count_label = self.tx(TextKey::NodeCount);
        let resolved_asset_count_label = self.tx(TextKey::ResolvedAssetCount);
        let default_volume_label = self.tx(TextKey::DefaultVolume);
        let fade_in_label = self.tx(TextKey::FadeInSeconds);
        let fade_out_label = self.tx(TextKey::FadeOutSeconds);
        let events_label = self.tx(TextKey::Events);
        let buses_label = self.tx(TextKey::Buses);
        let snapshots_label = self.tx(TextKey::Snapshots);
        let parameters_label = self.tx(TextKey::Parameters);
        let kind_label = self.tx(TextKey::Kind);
        let spatial_label = self.tx(TextKey::Spatial);
        let parameter_scope_label = self.tx(TextKey::ParameterScope);
        let parameter_variants_label = self.tx(TextKey::ParameterVariants);
        let default_value_label = self.tx(TextKey::DefaultValue);
        let unsupported_parameter_type_label = self.tx(TextKey::UnsupportedParameterType);
        let mut changed = false;

        match self.selected_item {
            Some(SelectedItem::Event(event_id)) => {
                let Some(project) = self.loaded_project.as_mut() else {
                    return;
                };
                let Some(event) = project.events.iter_mut().find(|event| event.id == event_id)
                else {
                    ui.label(self.tx(TextKey::NoSelection));
                    return;
                };
                ui.label(RichText::new(events_label).strong());
                ui.label(format!("ID: {}", event.id.0));
                let mut event_name = event.name.to_string();
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut event_name)
                            .desired_width(f32::INFINITY)
                            .hint_text("event.name"),
                    )
                    .changed();
                if event.name.as_str() != event_name {
                    event.name = event_name.into();
                }
                ui.label(kind_label);
                egui::ComboBox::from_id_salt(("event_kind", event.id.0))
                    .selected_text(format_event_kind_display(self.locale, event.kind))
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut event.kind,
                                EventKind::OneShot,
                                format_event_kind_display(self.locale, EventKind::OneShot),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut event.kind,
                                EventKind::Persistent,
                                format_event_kind_display(self.locale, EventKind::Persistent),
                            )
                            .changed();
                    });
                ui.label(spatial_label);
                egui::ComboBox::from_id_salt(("event_spatial", event.id.0))
                    .selected_text(format_spatial_mode_display(self.locale, event.spatial))
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut event.spatial,
                                SpatialMode::None,
                                format_spatial_mode_display(self.locale, SpatialMode::None),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut event.spatial,
                                SpatialMode::TwoD,
                                format_spatial_mode_display(self.locale, SpatialMode::TwoD),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut event.spatial,
                                SpatialMode::ThreeD,
                                format_spatial_mode_display(self.locale, SpatialMode::ThreeD),
                            )
                            .changed();
                    });
                ui.label(format!("{}: {}", node_count_label, event.root.nodes.len()));
                ui.label(format!(
                    "{}: {}",
                    resolved_asset_count_label,
                    collect_event_asset_ids(event).len()
                ));
                ui.separator();
                draw_event_content_editor(
                    ui,
                    self.locale,
                    event,
                    &asset_options,
                    &enum_parameter_options,
                    &mut changed,
                );
            }
            Some(SelectedItem::Bus(bus_id)) => {
                let Some(project) = self.loaded_project.as_mut() else {
                    return;
                };
                let Some(bus) = project.buses.iter_mut().find(|bus| bus.id == bus_id) else {
                    ui.label(self.tx(TextKey::NoSelection));
                    return;
                };
                ui.label(RichText::new(buses_label).strong());
                ui.label(format!("ID: {}", bus.id.0));
                let mut bus_name = bus.name.to_string();
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut bus_name)
                            .desired_width(f32::INFINITY)
                            .hint_text("bus.name"),
                    )
                    .changed();
                if bus.name.as_str() != bus_name {
                    bus.name = bus_name.into();
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut bus.default_volume, 0.0..=2.0)
                            .text(default_volume_label),
                    )
                    .changed();
            }
            Some(SelectedItem::Snapshot(snapshot_id)) => {
                let Some(project) = self.loaded_project.as_mut() else {
                    return;
                };
                let Some(snapshot) = project
                    .snapshots
                    .iter_mut()
                    .find(|snapshot| snapshot.id == snapshot_id)
                else {
                    ui.label(self.tx(TextKey::NoSelection));
                    return;
                };
                ui.label(RichText::new(snapshots_label).strong());
                ui.label(format!("ID: {}", snapshot.id.0));
                let mut snapshot_name = snapshot.name.to_string();
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut snapshot_name)
                            .desired_width(f32::INFINITY)
                            .hint_text("snapshot.name"),
                    )
                    .changed();
                if snapshot.name.as_str() != snapshot_name {
                    snapshot.name = snapshot_name.into();
                }
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut snapshot.fade_in_seconds)
                            .speed(0.05)
                            .prefix(format!("{}: ", fade_in_label)),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut snapshot.fade_out_seconds)
                            .speed(0.05)
                            .prefix(format!("{}: ", fade_out_label)),
                    )
                    .changed();
                ui.label(format!("Targets: {}", snapshot.targets.len()));
            }
            Some(SelectedItem::Parameter(parameter_id)) => {
                let Some(project) = self.loaded_project.as_mut() else {
                    return;
                };
                let Some(parameter) = project
                    .parameters
                    .iter_mut()
                    .find(|parameter| parameter.id() == parameter_id)
                else {
                    ui.label(self.tx(TextKey::NoSelection));
                    return;
                };
                let Parameter::Enum(parameter) = parameter else {
                    ui.label(unsupported_parameter_type_label);
                    return;
                };

                ui.label(RichText::new(parameters_label).strong());
                ui.label(format!("ID: {}", parameter.id.0));

                let mut parameter_name = parameter.name.to_string();
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut parameter_name)
                            .desired_width(f32::INFINITY)
                            .hint_text("music_state"),
                    )
                    .changed();
                if parameter.name.as_str() != parameter_name {
                    parameter.name = parameter_name.into();
                }

                ui.label(parameter_scope_label);
                egui::ComboBox::from_id_salt(("parameter_scope", parameter.id.0))
                    .selected_text(format_parameter_scope_display(self.locale, parameter.scope))
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut parameter.scope,
                                ParameterScope::Global,
                                format_parameter_scope_display(self.locale, ParameterScope::Global),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut parameter.scope,
                                ParameterScope::Emitter,
                                format_parameter_scope_display(
                                    self.locale,
                                    ParameterScope::Emitter,
                                ),
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut parameter.scope,
                                ParameterScope::EventInstance,
                                format_parameter_scope_display(
                                    self.locale,
                                    ParameterScope::EventInstance,
                                ),
                            )
                            .changed();
                    });

                let mut variants_text = parameter
                    .variants
                    .iter()
                    .map(|variant| variant.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                ui.label(parameter_variants_label);
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut variants_text)
                            .desired_width(f32::INFINITY)
                            .hint_text("explore, combat, stealth"),
                    )
                    .changed();

                let parsed_variants = crate::content::parse_variant_list(&variants_text);
                if !parsed_variants.is_empty() {
                    let current_variants: Vec<String> = parameter
                        .variants
                        .iter()
                        .map(|variant| variant.to_string())
                        .collect();
                    if current_variants != parsed_variants {
                        parameter.variants =
                            parsed_variants.iter().cloned().map(Into::into).collect();
                    }
                }

                if !parameter
                    .variants
                    .iter()
                    .any(|variant| variant == &parameter.default_value)
                {
                    if let Some(first_variant) = parameter.variants.first() {
                        parameter.default_value = first_variant.clone();
                    }
                }

                ui.label(default_value_label);
                egui::ComboBox::from_id_salt(("parameter_default_value", parameter.id.0))
                    .selected_text(parameter.default_value.as_str())
                    .show_ui(ui, |ui| {
                        for variant in &parameter.variants {
                            changed |= ui
                                .selectable_value(
                                    &mut parameter.default_value,
                                    variant.clone(),
                                    variant.as_str(),
                                )
                                .changed();
                        }
                    });
            }
            None => {
                ui.label(self.tx(TextKey::NoSelection));
                ui.separator();
                self.draw_validation_report(ui);
            }
        }

        if changed {
            self.on_project_changed();
        }
    }

    pub(crate) fn draw_export_report(&self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new(self.tx(TextKey::LastExport)).strong());

            let Some(report) = &self.last_export else {
                ui.label(self.tx(TextKey::NoExportYet));
                return;
            };

            let status_label = if report.success {
                self.tx(TextKey::LastExportSuccess)
            } else {
                self.tx(TextKey::LastExportFailure)
            };
            let status_color = if report.success {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_RED
            };

            ui.label(RichText::new(status_label).color(status_color));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::CurrentBank),
                report.bank_name
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::OutputPath),
                report.output_path
            ));

            if let Some(error_message) = &report.error_message {
                ui.colored_label(Color32::LIGHT_RED, error_message);
                return;
            }

            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::EventCount),
                report.event_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::BusCount),
                report.bus_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::SnapshotCount),
                report.snapshot_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::AssetCount),
                report.asset_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::ResidentMediaCount),
                report.resident_media_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::StreamingMediaCount),
                report.streaming_media_count
            ));

            if let Some(file_size_bytes) = report.file_size_bytes {
                ui.label(format!(
                    "{}: {} B",
                    self.tx(TextKey::FileSizeBytes),
                    file_size_bytes
                ));
            }
        });
    }

    pub(crate) fn draw_open_project_window(&mut self, ctx: &egui::Context) {
        if !self.show_open_project_window {
            return;
        }

        let mut open = self.show_open_project_window;
        egui::Window::new(self.tx(TextKey::WindowOpenProject))
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                let project_path_hint = self.tx(TextKey::ProjectPathHint);
                ui.label(self.tx(TextKey::ProjectPath));
                ui.add(
                    TextEdit::singleline(&mut self.project_path)
                        .desired_width(f32::INFINITY)
                        .hint_text(project_path_hint),
                );
                ui.add_space(8.0);
                if ui.button(self.tx(TextKey::LoadProject)).clicked() {
                    self.load_project();
                }
                ui.separator();
                ui.label(RichText::new(self.tx(TextKey::RecentProjects)).strong());
                if self.recent_projects.is_empty() {
                    ui.label(self.tx(TextKey::NoRecentProjects));
                } else {
                    let recent_projects = self.recent_projects.clone();
                    for path in recent_projects {
                        if ui.button(path.as_str()).clicked() {
                            self.project_path = path;
                            self.load_project();
                        }
                    }
                }
            });
        self.show_open_project_window = open;
    }

    pub(crate) fn draw_export_bank_window(&mut self, ctx: &egui::Context) {
        if !self.show_export_bank_window {
            return;
        }

        let mut open = self.show_export_bank_window;
        egui::Window::new(self.tx(TextKey::WindowExportBank))
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                let Some(project) = &self.loaded_project else {
                    ui.label(self.tx(TextKey::LoadProjectFirst));
                    return;
                };
                let Some(selected_bank_name) = self.selected_bank_name.clone() else {
                    ui.label(self.tx(TextKey::SelectBankFirst));
                    return;
                };
                let Some(bank) = project.bank_named(&selected_bank_name) else {
                    ui.label(self.tx(TextKey::SelectedBankMissing));
                    return;
                };

                ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank.name));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::EventCount),
                    bank.events.len()
                ));
                let output_path_hint = self.tx(TextKey::OutputPathHint);
                ui.label(self.tx(TextKey::OutputPath));
                ui.add(
                    TextEdit::singleline(&mut self.export_path)
                        .desired_width(f32::INFINITY)
                        .hint_text(output_path_hint),
                );
                ui.horizontal(|ui| {
                    let can_export =
                        self.validation_report.can_export() && !self.export_path.trim().is_empty();
                    if ui
                        .add_enabled(
                            can_export,
                            egui::Button::new(self.tx(TextKey::ExportCompiledBank)),
                        )
                        .clicked()
                    {
                        self.export_selected_bank();
                    }
                    if ui
                        .button(self.tx(TextKey::ResetDefaultExportPath))
                        .clicked()
                    {
                        self.export_path = self.suggest_export_path(&selected_bank_name);
                    }
                });
                ui.separator();
                self.draw_export_report(ui);
            });
        self.show_export_bank_window = open;
    }

    pub(crate) fn draw_bank_events_window(&mut self, ctx: &egui::Context) {
        if !self.show_bank_events_window {
            return;
        }

        let mut open = self.show_bank_events_window;
        egui::Window::new(self.tx(TextKey::WindowBankEvents))
            .open(&mut open)
            .default_width(560.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                self.draw_event_bank_editor(ui);
            });
        self.show_bank_events_window = open;
    }

    pub(crate) fn draw_diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.show_diagnostics_window {
            return;
        }

        let mut open = self.show_diagnostics_window;
        egui::Window::new(self.tx(TextKey::WindowDiagnostics))
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                self.draw_validation_report(ui);
            });
        self.show_diagnostics_window = open;
    }

    pub(crate) fn draw_log_window(&mut self, ctx: &egui::Context) {
        if !self.show_log_window {
            return;
        }

        let mut open = self.show_log_window;
        egui::Window::new(self.tx(TextKey::WindowLog))
            .open(&mut open)
            .default_width(520.0)
            .default_height(260.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(self.tx(TextKey::Log));
                    if ui.button(self.tx(TextKey::Clear)).clicked() {
                        self.logs.clear();
                    }
                });
                ui.separator();
                if self.logs.is_empty() {
                    ui.label(self.tx(TextKey::NoLogs));
                    return;
                }
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for entry in &self.logs {
                            let color = match entry.level {
                                LogLevel::Info => Color32::LIGHT_GREEN,
                                LogLevel::Error => Color32::LIGHT_RED,
                            };
                            ui.label(
                                RichText::new(format!("[{}] {}", entry.timestamp, entry.message))
                                    .color(color),
                            );
                        }
                    });
            });
        self.show_log_window = open;
    }

    pub(crate) fn draw_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about_window {
            return;
        }

        let mut open = self.show_about_window;
        egui::Window::new(self.tx(TextKey::WindowAbout))
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.heading(self.tx(TextKey::WelcomeTitle));
                ui.label(self.tx(TextKey::AboutText));
            });
        self.show_about_window = open;
    }

    fn draw_event_bank_editor(&mut self, ui: &mut egui::Ui) {
        let Some(project) = &self.loaded_project else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.clone() else {
            return;
        };

        let Some(bank) = project.bank_named(&bank_name) else {
            return;
        };

        let bank_event_ids = bank.events.clone();
        let bank_bus_ids = bank.buses.clone();
        let bank_snapshot_ids = bank.snapshots.clone();
        let current_bank_name = bank.name.to_string();
        let current_bank_events: Vec<(String, sonara_model::EventId)> = project
            .events
            .iter()
            .filter(|event| bank_event_ids.contains(&event.id))
            .map(|event| (event.name.to_string(), event.id))
            .collect();
        let available_events: Vec<(String, sonara_model::EventId)> = project
            .events
            .iter()
            .filter(|event| !bank_event_ids.contains(&event.id))
            .map(|event| (event.name.to_string(), event.id))
            .collect();
        let current_bank_buses: Vec<(String, sonara_model::BusId)> = project
            .buses
            .iter()
            .filter(|bus| bank_bus_ids.contains(&bus.id))
            .map(|bus| (bus.name.to_string(), bus.id))
            .collect();
        let available_buses: Vec<(String, sonara_model::BusId)> = project
            .buses
            .iter()
            .filter(|bus| !bank_bus_ids.contains(&bus.id))
            .map(|bus| (bus.name.to_string(), bus.id))
            .collect();
        let current_bank_snapshots: Vec<(String, sonara_model::SnapshotId)> = project
            .snapshots
            .iter()
            .filter(|snapshot| bank_snapshot_ids.contains(&snapshot.id))
            .map(|snapshot| (snapshot.name.to_string(), snapshot.id))
            .collect();
        let available_snapshots: Vec<(String, sonara_model::SnapshotId)> = project
            .snapshots
            .iter()
            .filter(|snapshot| !bank_snapshot_ids.contains(&snapshot.id))
            .map(|snapshot| (snapshot.name.to_string(), snapshot.id))
            .collect();

        let mut event_to_remove = None;
        let mut event_to_add = None;
        let mut bus_to_remove = None;
        let mut bus_to_add = None;
        let mut snapshot_to_remove = None;
        let mut snapshot_to_add = None;
        let can_create_event = !project.assets.is_empty();
        let mut should_create_event = false;
        let mut should_create_bus = false;
        let mut should_create_snapshot = false;
        let enum_parameters = self.enum_parameter_options();
        let project_assets: Vec<String> = project
            .assets
            .iter()
            .map(|asset| asset.name.to_string())
            .collect();
        let mut should_create_asset = false;
        let mut should_create_parameter = false;

        ui.group(|ui| {
            ui.label(RichText::new(self.tx(TextKey::BankContentsEditor)).strong());

            ui.label(RichText::new(self.tx(TextKey::CreateObjects)).strong());
            ui.horizontal(|ui| {
                ui.label(self.tx(TextKey::NewAssetPath));
                ui.add(
                    TextEdit::singleline(&mut self.new_asset_path)
                        .desired_width(340.0)
                        .hint_text("audio/music/explore_loop.wav"),
                );
                if ui.button(self.tx(TextKey::CreateAsset)).clicked() {
                    should_create_asset = true;
                }
            });
            ui.label(self.tx(TextKey::AssetImportHint));
            ui.horizontal(|ui| {
                ui.label(self.tx(TextKey::NewEventName));
                ui.add(
                    TextEdit::singleline(&mut self.new_event_name)
                        .desired_width(180.0)
                        .hint_text("player.new_event"),
                );
                if ui
                    .add_enabled(
                        can_create_event,
                        egui::Button::new(self.tx(TextKey::CreateEvent)),
                    )
                    .clicked()
                {
                    should_create_event = true;
                }
            });
            if !can_create_event {
                ui.label(self.tx(TextKey::CreateEventNeedsAsset));
            }
            ui.horizontal(|ui| {
                ui.label(self.tx(TextKey::NewBusName));
                ui.add(
                    TextEdit::singleline(&mut self.new_bus_name)
                        .desired_width(180.0)
                        .hint_text("sfx"),
                );
                if ui.button(self.tx(TextKey::CreateBus)).clicked() {
                    should_create_bus = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(self.tx(TextKey::NewSnapshotName));
                ui.add(
                    TextEdit::singleline(&mut self.new_snapshot_name)
                        .desired_width(180.0)
                        .hint_text("combat"),
                );
                if ui.button(self.tx(TextKey::CreateSnapshot)).clicked() {
                    should_create_snapshot = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(self.tx(TextKey::NewParameterName));
                ui.add(
                    TextEdit::singleline(&mut self.new_parameter_name)
                        .desired_width(180.0)
                        .hint_text("music_state"),
                );
                ui.label(self.tx(TextKey::NewParameterVariants));
                ui.add(
                    TextEdit::singleline(&mut self.new_parameter_variants)
                        .desired_width(220.0)
                        .hint_text("explore, combat, stealth"),
                );
                if ui.button(self.tx(TextKey::CreateEnumParameter)).clicked() {
                    should_create_parameter = true;
                }
            });
            ui.label(self.tx(TextKey::EnumParameterHint));

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::ProjectAssets)).strong());
            if project_assets.is_empty() {
                ui.label(self.tx(TextKey::NoAssetsInProject));
            } else {
                for asset_name in &project_assets {
                    ui.label(asset_name);
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::ProjectParameters)).strong());
            if enum_parameters.is_empty() {
                ui.label(self.tx(TextKey::NoParameters));
            } else {
                for parameter in &enum_parameters {
                    ui.horizontal(|ui| {
                        let selected =
                            self.selected_item == Some(SelectedItem::Parameter(parameter.id));
                        if ui.selectable_label(selected, &parameter.name).clicked() {
                            self.selected_item = Some(SelectedItem::Parameter(parameter.id));
                        }
                        ui.label(format!(
                            "{}: {}",
                            self.tx(TextKey::VariantCount),
                            parameter.variants.len()
                        ));
                    });
                }
            }

            ui.separator();
            ui.label(self.tx(TextKey::CurrentBankEvents));

            if current_bank_events.is_empty() {
                ui.label(self.tx(TextKey::NoEvents));
            } else {
                for (event_name, event_id) in &current_bank_events {
                    ui.horizontal(|ui| {
                        let selected = self.selected_item == Some(SelectedItem::Event(*event_id));
                        if ui.selectable_label(selected, event_name).clicked() {
                            self.selected_item = Some(SelectedItem::Event(*event_id));
                        }
                        if ui.button(self.tx(TextKey::RemoveFromBank)).clicked() {
                            event_to_remove = Some((event_name.clone(), *event_id));
                        }
                    });
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::AvailableEvents)).strong());

            if available_events.is_empty() {
                ui.label(self.tx(TextKey::NoAvailableEvents));
            } else {
                for (event_name, event_id) in &available_events {
                    ui.horizontal(|ui| {
                        ui.label(event_name);
                        if ui.button(self.tx(TextKey::AddToBank)).clicked() {
                            event_to_add = Some((event_name.clone(), *event_id));
                        }
                    });
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::CurrentBankBuses)).strong());

            if current_bank_buses.is_empty() {
                ui.label(self.tx(TextKey::NoBuses));
            } else {
                for (bus_name, bus_id) in &current_bank_buses {
                    ui.horizontal(|ui| {
                        let selected = self.selected_item == Some(SelectedItem::Bus(*bus_id));
                        if ui.selectable_label(selected, bus_name).clicked() {
                            self.selected_item = Some(SelectedItem::Bus(*bus_id));
                        }
                        if ui.button(self.tx(TextKey::RemoveFromBank)).clicked() {
                            bus_to_remove = Some(*bus_id);
                        }
                    });
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::AvailableBuses)).strong());

            if available_buses.is_empty() {
                ui.label(self.tx(TextKey::NoAvailableBuses));
            } else {
                for (bus_name, bus_id) in &available_buses {
                    ui.horizontal(|ui| {
                        ui.label(bus_name);
                        if ui.button(self.tx(TextKey::AddToBank)).clicked() {
                            bus_to_add = Some(*bus_id);
                        }
                    });
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::CurrentBankSnapshots)).strong());

            if current_bank_snapshots.is_empty() {
                ui.label(self.tx(TextKey::NoSnapshots));
            } else {
                for (snapshot_name, snapshot_id) in &current_bank_snapshots {
                    ui.horizontal(|ui| {
                        let selected =
                            self.selected_item == Some(SelectedItem::Snapshot(*snapshot_id));
                        if ui.selectable_label(selected, snapshot_name).clicked() {
                            self.selected_item = Some(SelectedItem::Snapshot(*snapshot_id));
                        }
                        if ui.button(self.tx(TextKey::RemoveFromBank)).clicked() {
                            snapshot_to_remove = Some(*snapshot_id);
                        }
                    });
                }
            }

            ui.separator();
            ui.label(RichText::new(self.tx(TextKey::AvailableSnapshots)).strong());

            if available_snapshots.is_empty() {
                ui.label(self.tx(TextKey::NoAvailableSnapshots));
            } else {
                for (snapshot_name, snapshot_id) in &available_snapshots {
                    ui.horizontal(|ui| {
                        ui.label(snapshot_name);
                        if ui.button(self.tx(TextKey::AddToBank)).clicked() {
                            snapshot_to_add = Some(*snapshot_id);
                        }
                    });
                }
            }
        });

        if let Some((event_name, event_id)) = event_to_remove {
            self.remove_event_from_selected_bank(event_id);
            self.status_message = self.tr(TextTemplate::RemovedEventFromBank {
                event_name: event_name.clone(),
                bank_name: current_bank_name.clone(),
            });
            self.push_info_log(self.tr(TextTemplate::RemovedEventFromBank {
                event_name,
                bank_name: current_bank_name.clone(),
            }));
        }

        if let Some((event_name, event_id)) = event_to_add {
            self.add_event_to_selected_bank(event_id);
            self.status_message = self.tr(TextTemplate::AddedEventToBank {
                event_name: event_name.clone(),
                bank_name: current_bank_name.clone(),
            });
            self.push_info_log(self.tr(TextTemplate::AddedEventToBank {
                event_name,
                bank_name: current_bank_name,
            }));
        }

        if let Some(bus_id) = bus_to_remove {
            self.remove_bus_from_selected_bank(bus_id);
        }

        if let Some(bus_id) = bus_to_add {
            self.add_bus_to_selected_bank(bus_id);
        }

        if let Some(snapshot_id) = snapshot_to_remove {
            self.remove_snapshot_from_selected_bank(snapshot_id);
        }

        if let Some(snapshot_id) = snapshot_to_add {
            self.add_snapshot_to_selected_bank(snapshot_id);
        }

        if should_create_event {
            self.create_event_in_selected_bank();
        }

        if should_create_bus {
            self.create_bus_in_selected_bank();
        }

        if should_create_snapshot {
            self.create_snapshot_in_selected_bank();
        }

        if should_create_asset {
            self.create_asset_in_project();
        }

        if should_create_parameter {
            self.create_enum_parameter();
        }
    }
}
