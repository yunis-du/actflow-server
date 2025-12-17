use std::sync::{Arc, Mutex};

use actflow::{ChannelEvent, ChannelOptions, Engine};
use anyhow::Result;
use log::{error, info};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Response, Status};

use crate::proto::{
    RunWorkflowRequest, StopWorkflowRequest, StopWorkflowResponse, WorkflowEvent, workflow_event::Event as ProtoEvent,
    workflow_service_server::WorkflowService,
};

pub struct WorkflowServer {
    engine: Arc<Engine>,
}

impl WorkflowServer {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self {
            engine,
        }
    }
}

type RR<T> = Result<Response<T>, Status>;

#[tonic::async_trait]
impl WorkflowService for WorkflowServer {
    type RunWorkflowStream = ReceiverStream<Result<WorkflowEvent, Status>>;

    async fn run_workflow(
        &self,
        request: tonic::Request<RunWorkflowRequest>,
    ) -> RR<Self::RunWorkflowStream> {
        let request = request.into_inner();

        let workflow_model: actflow::WorkflowModel = serde_json::from_str(&request.workflow_model)
            .map_err(|e| Status::invalid_argument(format!("Invalid workflow model: {}", e)))?;
        let wid = workflow_model.id.clone();

        info!("running workflow: {}", wid);

        let porc = self
            .engine
            .build_workflow_process(&workflow_model)
            .map_err(|e| Status::internal(format!("Failed to build workflow process: {}", e)))?;
        let pid = porc.id();

        let (tx, rx) = mpsc::channel(100);
        let tx = Arc::new(Mutex::new(Some(tx)));

        let tx_event = tx.clone();
        ChannelEvent::channel(self.engine.channel(), ChannelOptions::with_pid(pid.to_owned())).on_event(move |event| {
            handle_workflow_events(&tx_event, &wid, &event);
        });

        let tx_log = tx.clone();
        ChannelEvent::channel(self.engine.channel(), ChannelOptions::with_pid(pid.to_owned())).on_log(move |log| {
            handle_workflow_logs(&tx_log, log);
        });

        porc.start();

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn stop_workflow(
        &self,
        request: tonic::Request<StopWorkflowRequest>,
    ) -> RR<StopWorkflowResponse> {
        let pid = request.into_inner().pid;
        match self.engine.stop(&pid) {
            Ok(()) => Ok(Response::new(StopWorkflowResponse {
                success: true,
                err_msg: "".to_string(),
            })),
            Err(err) => Ok(Response::new(StopWorkflowResponse {
                success: false,
                err_msg: err.to_string(),
            })),
        }
    }
}

type WorkflowEventTx = Arc<Mutex<Option<mpsc::Sender<Result<WorkflowEvent, Status>>>>>;

fn handle_workflow_events(
    tx: &WorkflowEventTx,
    workflow_id: &str,
    event: &actflow::Event<actflow::Message>,
) {
    // Check if the event is terminal
    let is_terminal = matches!(
        &event.event,
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Succeeded)
            | actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Failed(_))
            | actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Aborted(_))
    );

    let workflow_event = match &event.event {
        // Workflow events
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Start(_)) => WorkflowEvent {
            event: Some(ProtoEvent::WorkflowStart(crate::proto::WorkflowStart {
                pid: event.pid.clone(),
            })),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Succeeded) => WorkflowEvent {
            event: Some(ProtoEvent::WorkflowSuccess(crate::proto::WorkflowSuccess {
                pid: event.pid.clone(),
            })),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Failed(err)) => WorkflowEvent {
            event: Some(ProtoEvent::WorkflowFailure(crate::proto::WorkflowFailure {
                pid: event.pid.clone(),
                err_msg: err.error.clone(),
            })),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Aborted(aborted)) => WorkflowEvent {
            event: Some(ProtoEvent::WorkflowAbort(crate::proto::WorkflowAbort {
                pid: event.pid.clone(),
                reason: aborted.reason.clone(),
            })),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Paused(paused)) => WorkflowEvent {
            event: Some(ProtoEvent::WorkflowPause(crate::proto::WorkflowPause {
                pid: event.pid.clone(),
                reason: paused.reason.clone(),
            })),
        },
        // Node events
        actflow::GraphEvent::Node(actflow::NodeEvent::Running(_)) => WorkflowEvent {
            event: Some(ProtoEvent::NodeRunning(crate::proto::NodeRunning {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Stopped(_)) => WorkflowEvent {
            event: Some(ProtoEvent::NodeStopped(crate::proto::NodeStopped {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Paused(_)) => WorkflowEvent {
            event: Some(ProtoEvent::NodePaused(crate::proto::NodePaused {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
                reason: "Paused by user".to_string(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Skipped) => WorkflowEvent {
            event: Some(ProtoEvent::NodeSkipped(crate::proto::NodeSkipped {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Succeeded(_)) => WorkflowEvent {
            event: Some(ProtoEvent::NodeSuccess(crate::proto::NodeSuccess {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Error(err)) => WorkflowEvent {
            event: Some(ProtoEvent::NodeError(crate::proto::NodeError {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
                err_msg: err.to_string(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Retry) => WorkflowEvent {
            event: Some(ProtoEvent::NodeRetry(crate::proto::NodeRetry {
                pid: event.pid.clone(),
                nid: event.nid.clone(),
            })),
        },
    };

    if is_terminal {
        if let Some(sender) = tx.lock().unwrap().take() {
            if let Err(e) = sender.try_send(Ok(workflow_event)) {
                error!("failed to send workflow event: {}", e);
            }
            info!("workflow [{}] execution completed", workflow_id);
        }
    } else {
        if let Some(sender) = tx.lock().unwrap().as_ref() {
            if let Err(e) = sender.try_send(Ok(workflow_event)) {
                error!("failed to send workflow event: {}", e);
            }
        }
    }
}

fn handle_workflow_logs(
    tx: &WorkflowEventTx,
    log: &actflow::Log,
) {
    let log_event = WorkflowEvent {
        event: Some(ProtoEvent::NodeLog(crate::proto::NodeLog {
            pid: log.pid.clone(),
            nid: log.nid.clone(),
            content: log.content.clone(),
            timestamp: log.timestamp,
        })),
    };
    if let Some(sender) = tx.lock().unwrap().as_ref() {
        if let Err(e) = sender.try_send(Ok(log_event)) {
            error!("failed to send workflow log event: {}", e);
        }
    }
}
