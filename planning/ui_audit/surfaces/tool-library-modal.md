surface: tool-library-modal
file: crates/rs_cam_viz/src/ui/tool_library_modal.rs
kind: modal
job: Browse, preview, edit, organise, and import per-user tool catalogs into the project.
opens-from: AppEvent opening state.tool_library_modal (e.g. Add Tool ▸ From library); a centered egui::Window.
controls:
  - create-tool-catalog
  - select-tool-catalog
  - rename-tool-catalog
  - dedupe-tool-catalog
  - delete-tool-catalog
  - search-all-catalogs
  - filter-tools
  - select-library-tool
  - view-tool-preview (read-only)
  - add-tool-from-library
  - edit-library-tool
  - delete-library-tool
  - move-library-tool
  - update-library-tool
  - (edit form re-exposes all tool-* geometry controls via draw_tool_fields)
reads-state: state.tool_library_modal.catalogs (name -> ToolCatalog.tools), egui temp ToolLibraryView (selection, filter, search_all, editing, draft, confirm flags, buffers)
writes-state: ToolLibraryView (temp only); persistent changes routed via AppEvent (CreateToolCatalog, RenameToolCatalog, DedupeToolCatalog, DeleteToolCatalog, AddToolFromLibrary, DeleteLibraryTool, MoveLibraryTool, UpdateLibraryTool, CloseToolLibrary)
confusable-with: tool-properties-panel (edit form is the same draw_tool_fields widget; preview + readonly grid mirror the project tool editor)
recommendation-sources-touched: none
health: green — well-structured master/detail modal (catalog list -> tool list -> preview/detail/edit) with clean summary->detail progression and event-only mutation; the only smell is that its edit form is indistinguishable from the project tool editor (P4, noted on that surface).
