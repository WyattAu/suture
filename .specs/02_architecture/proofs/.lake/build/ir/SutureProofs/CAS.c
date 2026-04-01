// Lean compiler output
// Module: SutureProofs.CAS
// Imports: public import Init public import SutureProofs.Foundations
#include <lean/lean.h>
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wunused-label"
#elif defined(__GNUC__) && !defined(__CLANG__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-label"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#endif
#ifdef __cplusplus
extern "C" {
#endif
extern lean_object* l_ByteArray_empty;
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_blake3(lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_blake3___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_blake3(lean_object* x_1) {
_start:
{
lean_object* x_2; 
x_2 = l_ByteArray_empty;
return x_2;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_blake3___boxed(lean_object* x_1) {
_start:
{
lean_object* x_2; 
x_2 = lp_suture_x2dproofs_Suture_blake3(x_1);
lean_dec_ref(x_1);
return x_2;
}
}
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_suture_x2dproofs_SutureProofs_Foundations(uint8_t builtin);
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_suture_x2dproofs_SutureProofs_CAS(uint8_t builtin) {
lean_object * res;
if (_G_initialized) return lean_io_result_mk_ok(lean_box(0));
_G_initialized = true;
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_suture_x2dproofs_SutureProofs_Foundations(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
return lean_io_result_mk_ok(lean_box(0));
}
#ifdef __cplusplus
}
#endif
