use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

pub type RequestIdLayer = (SetRequestIdLayer<MakeRequestUuid>, PropagateRequestIdLayer);

pub fn request_id_layer() -> RequestIdLayer {
    (
        SetRequestIdLayer::x_request_id(MakeRequestUuid),
        PropagateRequestIdLayer::x_request_id(),
    )
}
