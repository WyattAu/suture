# Partner Integration Roadmap

## 1. Notion Integration

### Design
- Bidirectional sync between Suture repos and Notion pages
- Notion pages → Suture structured files (Markdown)
- Merge conflicts resolved semantically

### Implementation
- Notion API client: `GET /v1/blocks/{id}/children`, `PATCH /v1/blocks/{id}`
- Map Notion block types to Markdown AST
- Watch for Notion webhook events
- Store Notion API token in org settings

### User Flow
1. User connects Notion workspace via OAuth
2. Selects pages to sync
3. Suture imports pages as `.md` files
4. Changes in Suture are synced back to Notion

## 2. Google Sheets Integration

### Design
- Import/export Google Sheets as CSV (already supported format)
- Real-time sync via Google Sheets API webhooks
- Cell-level conflict resolution

### Implementation
- Google Sheets API v4 client
- OAuth2 for Google authorization
- Watch for changes via `sheets.push` notifications
- Convert CSV ↔ Google Sheets rows/columns

## 3. Airtable Integration

### Design
- Import Airtable bases as JSON (Airtable's native format)
- Sync records bidirectionally
- Schema evolution support

### Implementation
- Airtable REST API: `GET /v0/{base_id}/{table}`
- Webhook subscriptions for change notifications
- Field type mapping: Airtable types → JSON Schema

## 4. Confluence Integration

### Design
- Import Confluence pages as Markdown
- Sync edits back to Confluence
- Preserve Confluence-specific markup (macros, includes)

### Implementation
- Confluence Cloud REST API v2
- Storage format API: `GET /rest/api/content/{id}`
- Convert Confluence XHTML → Markdown → Suture semantic merge

## Plugin Architecture for Integrations

All partner integrations should be implemented as WASM plugins:

```rust
pub trait PartnerSyncPlugin {
    fn name(&self) -> &str;
    fn supported_formats(&self) -> &[&str];
    fn fetch_remote(&self, config: &Value) -> Result<Vec<(String, Vec<u8>)>>;
    fn push_local(&self, config: &Value, files: &[(String, Vec<u8>)]) -> Result<()>;
    fn watch_changes(&self, config: &Value) -> Result<()>;
}
```
