// cyril-lki9 P2 probe: deserialize captured KAS wake frames through the exact
// type path cyril's converter uses (agent_client_protocol::schema::v1) and print
// whether _meta.kiro.agentInitiated survives on each update kind.
use agent_client_protocol::schema::v1 as acp;
use std::io::BufRead;
fn main() {
    let path = std::env::args().nth(1).expect("capture path");
    let mut counts = std::collections::BTreeMap::<String, (u32, u32)>::new();
    for line in std::io::BufReader::new(std::fs::File::open(path).expect("open")).lines() {
        let v: serde_json::Value = serde_json::from_str(&line.expect("line")).expect("json");
        let msg = &v["msg"];
        if msg["method"] != "session/update" { continue; }
        let raw_tagged = msg["params"]["update"]["_meta"]["kiro"]["agentInitiated"] == true;
        if !raw_tagged { continue; }
        let n: acp::SessionNotification = match serde_json::from_value(msg["params"].clone()) {
            Ok(n) => n, Err(e) => { println!("DESERIALIZE-ERR {e}"); continue; }
        };
        let (kind, meta) = match &n.update {
            acp::SessionUpdate::AgentMessageChunk(c) => ("agent_message_chunk", c.meta.clone()),
            acp::SessionUpdate::ToolCall(t) => ("tool_call", t.meta.clone()),
            acp::SessionUpdate::ToolCallUpdate(t) => ("tool_call_update", t.meta.clone()),
            other => { println!("OTHER {:?}", std::mem::discriminant(other)); continue; }
        };
        let kept = meta.as_ref().and_then(|m| m.get("kiro")).and_then(|k| k.get("agentInitiated")) == Some(&serde_json::Value::Bool(true))
            && meta.as_ref().and_then(|m| m.get("kiro")).and_then(|k| k.get("agentInitiatedReason")).is_some();
        let e = counts.entry(kind.to_string()).or_default();
        e.0 += 1; if kept { e.1 += 1; }
    }
    for (k, (seen, kept)) in counts { println!("{k}: raw-tagged={seen} tag-retained-after-deserialize={kept}"); }
}
