use actflow::{ChannelEvent, ChannelOptions, Engine};
use anyhow::Result;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Response, Status};

use crate::proto::{
    RunWorkflowRequest, StopWorkflowRequest, StopWorkflowResponse, WorkflowEvent, workflow_service_server::WorkflowService,
};

pub struct WorkflowServer {
    engine: Engine,
}

impl WorkflowServer {
    pub fn new(engine: Engine) -> Self {
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
        let workflow_model = request.workflow_model.ok_or_else(|| Status::invalid_argument("Missing workflow model"))?;

        let workflow_model: actflow::WorkflowModel = serde_json::from_value(struct_to_json_value(workflow_model))
            .map_err(|e| Status::invalid_argument(format!("Invalid workflow model: {}", e)))?;

        let porc = self
            .engine
            .build_workflow_process(&workflow_model)
            .map_err(|e| Status::internal(format!("Failed to build workflow process: {}", e)))?;
        let pid = porc.id();

        let (tx, rx) = mpsc::channel(100);

        let tx_event = tx.clone();
        ChannelEvent::channel(self.engine.channel(), ChannelOptions::with_pid(pid.to_owned())).on_event_async(move |event| {
            let tx = tx_event.clone();
            let event = event.clone();
            Box::pin(async move {
                handle_workflow_events(&tx, &event).await;
            })
        });

        let tx_log = tx;
        ChannelEvent::channel(self.engine.channel(), ChannelOptions::with_pid(pid.to_owned())).on_log_async(move |log| {
            let tx = tx_log.clone();
            let log = log.clone();
            Box::pin(async move {
                handle_workflow_logs(&tx, &log).await;
            })
        });

        porc.start();

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn stop_workflow(
        &self,
        request: tonic::Request<StopWorkflowRequest>,
    ) -> RR<StopWorkflowResponse> {
        let pid = request.into_inner().process_id;
        match self.engine.stop(&pid) {
            Ok(()) => Ok(Response::new(StopWorkflowResponse {
                success: true,
                error_message: "".to_string(),
            })),
            Err(err) => Ok(Response::new(StopWorkflowResponse {
                success: false,
                error_message: err.to_string(),
            })),
        }
    }
}

type WorkflowEventTx = mpsc::Sender<Result<WorkflowEvent, Status>>;

async fn handle_workflow_events(
    tx: &WorkflowEventTx,
    event: &actflow::Event<actflow::Message>,
) {
    let workflow_event = match &event.event {
        // Workflow events
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Start(_)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::WorkflowStart(
                crate::proto::WorkflowStart {
                    process_id: event.pid.clone(),
                },
            )),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Succeeded) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::WorkflowSuccess(
                crate::proto::WorkflowSuccess {
                    process_id: event.pid.clone(),
                },
            )),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Failed(err)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::WorkflowFailure(
                crate::proto::WorkflowFailure {
                    process_id: event.pid.clone(),
                    error_message: err.error.clone(),
                },
            )),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Aborted(aborted)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::WorkflowAbort(
                crate::proto::WorkflowAbort {
                    process_id: event.pid.clone(),
                    reason: aborted.reason.clone(),
                },
            )),
        },
        actflow::GraphEvent::Workflow(actflow::WorkflowEvent::Paused(paused)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::WorkflowPause(
                crate::proto::WorkflowPause {
                    process_id: event.pid.clone(),
                    reason: paused.reason.clone(),
                },
            )),
        },
        // Node events
        actflow::GraphEvent::Node(actflow::NodeEvent::Running(_)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeRunning(crate::proto::NodeRunning {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Stopped(_)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeStopped(crate::proto::NodeStopped {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Paused(_)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodePaused(crate::proto::NodePaused {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
                reason: String::new(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Skipped) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeSkipped(crate::proto::NodeSkipped {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Succeeded(_)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeSuccess(crate::proto::NodeSuccess {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Error(err)) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeError(crate::proto::NodeError {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
                error_message: err.to_string(),
            })),
        },
        actflow::GraphEvent::Node(actflow::NodeEvent::Retry) => WorkflowEvent {
            event: Some(crate::proto::workflow_event::Event::NodeRetry(crate::proto::NodeRetry {
                process_id: event.pid.clone(),
                node_id: event.nid.clone(),
            })),
        },
    };

    if let Err(e) = tx.send(Ok(workflow_event)).await {
        eprintln!("Failed to send workflow event: {}", e);
    }
}

async fn handle_workflow_logs(
    tx: &WorkflowEventTx,
    log: &actflow::Log,
) {
    let log_event = WorkflowEvent {
        event: Some(crate::proto::workflow_event::Event::NodeLog(crate::proto::NodeLog {
            process_id: log.pid.clone(),
            node_id: log.nid.clone(),
            log_message: log.content.clone(),
            timestamp: log.timestamp,
        })),
    };
    if let Err(e) = tx.send(Ok(log_event)).await {
        eprintln!("Failed to send workflow log event: {}", e);
    }
}

/// Convert prost_types::Struct to serde_json::Value
fn struct_to_json_value(s: prost_types::Struct) -> serde_json::Value {
    serde_json::Value::Object(s.fields.into_iter().map(|(k, v)| (k, prost_value_to_json_value(v))).collect())
}

fn prost_value_to_json_value(v: prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;
    match v.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::NumberValue(n)) => serde_json::json!(n),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::StructValue(s)) => struct_to_json_value(s),
        Some(Kind::ListValue(l)) => serde_json::Value::Array(l.values.into_iter().map(prost_value_to_json_value).collect()),
        None => serde_json::Value::Null,
    }
}
