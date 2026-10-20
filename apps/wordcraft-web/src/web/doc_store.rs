//! Plume: the documents opened or saved in the browser, kept in IndexedDB for File › Open's
//! Recent list, so they open again on a later visit (the browser gives a page no access to the
//! files themselves). At most [`MAX_DOCS`], each at most [`MAX_BYTES`]; the oldest are dropped.
//! Every failure is logged and skipped: a document that can't be kept still saves and opens.

use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use web_sys::IdbTransactionMode;

use super::font_store::{done, js, open};

pub(super) const STORE: &str = "documents";
/// Most documents kept.
const MAX_DOCS: usize = 12;
/// Largest document kept.
const MAX_BYTES: usize = 64 << 20;
/// Where the kept documents' names are listed, newest first (`localStorage`).
const ORDER_KEY: &str = "plume-recent-documents";

fn order() -> Vec<String> {
    crate::web::local_storage()
        .and_then(|s| s.get_item(ORDER_KEY).ok().flatten())
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default()
}

fn set_order(names: &[String]) {
    if let (Some(s), Ok(t)) = (crate::web::local_storage(), serde_json::to_string(names)) {
        let _ = s.set_item(ORDER_KEY, &t);
    }
}

async fn remember_async(name: String, bytes: Vec<u8>) -> Result<(), String> {
    let db = open().await?;
    let store = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite).map_err(js)?.object_store(STORE).map_err(js)?;
    done(&store.put_with_key(&js_sys::Uint8Array::from(bytes.as_slice()), &JsValue::from_str(&name)).map_err(js)?).await?;
    let mut names = order();
    names.retain(|n| *n != name);
    names.insert(0, name);
    for old in names.split_off(MAX_DOCS.min(names.len())) {
        done(&store.delete(&JsValue::from_str(&old)).map_err(js)?).await?;
    }
    set_order(&names);
    Ok(())
}

/// Keep the document `name` (its bytes as opened or saved).
pub fn remember(name: &str, bytes: &[u8]) {
    if bytes.len() > MAX_BYTES {
        log::warn!("{name} is too large to keep for the Recent list");
        return;
    }
    let (name, bytes) = (name.to_string(), bytes.to_vec());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = remember_async(name.clone(), bytes).await {
            log::error!("couldn't keep {name} for the Recent list: {e}");
        }
    });
}

/// Open the kept document `name`: its bytes go to `inbox`, as a picked file's do.
pub fn open_stored(name: &str, inbox: wordcraft_ui_egui::Inbox, ctx: egui::Context) {
    let name = name.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        let r: Result<Vec<u8>, String> = async {
            let db = open().await?;
            let store = db.transaction_with_str(STORE).map_err(js)?.object_store(STORE).map_err(js)?;
            let v = done(&store.get(&JsValue::from_str(&name)).map_err(js)?).await?;
            let bytes = v.dyn_into::<js_sys::Uint8Array>().map_err(|_| "not kept in this browser any more".to_string())?;
            Ok(bytes.to_vec())
        }
        .await;
        match r {
            Ok(bytes) => {
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                ctx.request_repaint();
            }
            Err(e) => log::warn!("couldn't open {name} from the Recent list: {e}"),
        }
    });
}
