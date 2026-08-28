#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, AgentRun, AgentRunStatus, CanvasTransform, DefaultDesignerSession,
    DesignerSession, EditorView, InMemoryDesignerPersistence, InMemoryWorkspacePersistence,
    OperationId, PanelGeometry, PanelId, ProjectId, SessionContextUpdate, StudioDesign,
    UnsavedWork, WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceCommand, WorkspaceController,
    WorkspacePersistence, WorkspaceRecord, WorkspaceState, command_registry,
};

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn project_id() -> ProjectId {
    ProjectId::new("workbench-project").expect("valid project id")
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).expect("valid operation id")
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("workbench-test").expect("valid actor id"),
        kind: ActorKind::Human,
        display_name: "Workbench test".to_owned(),
    }
}

#[test]
fn switching_views_reads_one_shared_session_snapshot() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = DefaultDesignerSession::create(
            persistence,
            StudioDesign::empty(project_id(), "Workbench"),
            operation_id("create-workbench"),
            actor(),
            studio_design::UndoGroupId::new("create").expect("valid undo group"),
        )
        .await
        .expect("empty project creates");
        let run_id = operation_id("agent-run");
        let expected = session.update_context(SessionContextUpdate {
            canvas_transform: Some(CanvasTransform {
                zoom_milli: 1_250,
                offset_x: -40,
                offset_y: 12,
            }),
            runs: Some(vec![AgentRun {
                operation_id: run_id,
                status: AgentRunStatus::Running,
                progress_percent: 42,
            }]),
            unsaved_work: Some(UnsavedWork {
                dirty: true,
                buffer_id: Some("screen-main.studio".to_owned()),
            }),
            ..SessionContextUpdate::default()
        });

        let mut controller = WorkspaceController::default();
        let switched = controller
            .switch_to(EditorView::Workbench, &session)
            .expect("session state is available");
        assert_eq!(switched.view, EditorView::Workbench);
        assert_eq!(switched.session, expected);

        let switched_back = controller.toggle(&session).expect("toggle succeeds");
        assert_eq!(switched_back.view, EditorView::Focus);
        assert_eq!(switched_back.session, expected);
    });
}

#[test]
fn panel_geometry_is_independent_and_survives_persistence_round_trip() {
    block_on(async {
        let project_id = project_id();
        let persistence = InMemoryWorkspacePersistence::default();
        let mut state = WorkspaceState::new();
        state
            .focus
            .set_geometry(PanelId::Inspector, PanelGeometry::new(1, 2, 333, 444));
        state
            .workbench
            .set_geometry(PanelId::Inspector, PanelGeometry::new(10, 20, 555, 666));
        state.switch_to(EditorView::Workbench);
        let record = WorkspaceRecord {
            schema_version: WORKSPACE_STATE_SCHEMA_VERSION,
            project_id: project_id.clone(),
            state: state.clone(),
        };
        persistence.save(&record).await.expect("save succeeds");

        let reopened = persistence
            .load(&project_id)
            .await
            .expect("load succeeds")
            .expect("saved state exists");
        assert_eq!(reopened.state, state);
        assert_eq!(
            reopened
                .state
                .focus
                .panel(PanelId::Inspector)
                .geometry
                .width,
            333
        );
        assert_eq!(
            reopened
                .state
                .workbench
                .panel(PanelId::Inspector)
                .geometry
                .width,
            555
        );
    });
}

#[test]
fn every_workbench_surface_has_a_command_bar_and_keyboard_entry() {
    let registry = command_registry();
    assert_eq!(registry.len(), PanelId::all().len() + 1);
    for panel in PanelId::all() {
        let command = registry
            .iter()
            .find(|descriptor| descriptor.command.panel() == Some(panel))
            .expect("every panel has a command");
        assert!(!command.label.is_empty());
        assert!(!command.shortcut.is_empty());
    }
    assert!(
        registry
            .iter()
            .any(|descriptor| descriptor.command == WorkspaceCommand::ToggleView)
    );
}
