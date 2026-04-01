// Lean compiler output
// Module: SutureProofs.Foundations
// Imports: public import Init public import Mathlib.Data.Finset.Basic
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
lean_object* l_instDecidableEqString___boxed(lean_object*, lean_object*);
uint8_t lp_mathlib_Multiset_decidableMem___aux__1___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_StaticPatch_apply(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_identityPatch___lam__0(lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_identityPatch___lam__0___boxed(lean_object*);
static const lean_closure_object lp_suture_x2dproofs_Suture_identityPatch___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_closure_object) + sizeof(void*)*0, .m_other = 0, .m_tag = 245}, .m_fun = (void*)lp_suture_x2dproofs_Suture_identityPatch___lam__0___boxed, .m_arity = 1, .m_num_fixed = 0, .m_objs = {} };
static const lean_object* lp_suture_x2dproofs_Suture_identityPatch___closed__0 = (const lean_object*)&lp_suture_x2dproofs_Suture_identityPatch___closed__0_value;
static const lean_ctor_object lp_suture_x2dproofs_Suture_identityPatch___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 0}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_suture_x2dproofs_Suture_identityPatch___closed__0_value)}};
static const lean_object* lp_suture_x2dproofs_Suture_identityPatch___closed__1 = (const lean_object*)&lp_suture_x2dproofs_Suture_identityPatch___closed__1_value;
LEAN_EXPORT const lean_object* lp_suture_x2dproofs_Suture_identityPatch = (const lean_object*)&lp_suture_x2dproofs_Suture_identityPatch___closed__1_value;
uint8_t l_Option_instDecidableEq___redArg(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_suture_x2dproofs_Suture_actualTouchSet___lam__0(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_actualTouchSet___lam__0___boxed(lean_object*, lean_object*, lean_object*);
lean_object* lp_mathlib_Multiset_filter___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_actualTouchSet(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_compose___lam__0(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
uint8_t lean_usize_dec_eq(size_t, size_t);
size_t lean_usize_sub(size_t, size_t);
lean_object* lean_array_uget_borrowed(lean_object*, size_t);
uint8_t lp_mathlib_List_elem___at___00Mathlib_Meta_FunProp_logError_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1(lean_object*, size_t, size_t, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
lean_object* lean_array_mk(lean_object*);
lean_object* lean_array_get_size(lean_object*);
uint8_t lean_nat_dec_lt(lean_object*, lean_object*);
size_t lean_usize_of_nat(lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Multiset_ndunion___at___00Suture_compose_spec__0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_compose(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_StaticPatch_apply(lean_object* x_1, lean_object* x_2, lean_object* x_3) {
_start:
{
lean_object* x_4; lean_object* x_5; lean_object* x_6; uint8_t x_7; 
x_4 = lean_ctor_get(x_1, 0);
lean_inc(x_4);
x_5 = lean_ctor_get(x_1, 1);
lean_inc_ref(x_5);
lean_dec_ref(x_1);
x_6 = lean_alloc_closure((void*)(l_instDecidableEqString___boxed), 2, 0);
lean_inc_ref(x_3);
x_7 = lp_mathlib_Multiset_decidableMem___aux__1___redArg(x_6, x_3, x_4);
if (x_7 == 0)
{
lean_object* x_8; 
lean_dec_ref(x_5);
x_8 = lean_apply_1(x_2, x_3);
return x_8;
}
else
{
lean_object* x_9; 
lean_dec_ref(x_2);
x_9 = lean_apply_1(x_5, x_3);
return x_9;
}
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_identityPatch___lam__0(lean_object* x_1) {
_start:
{
lean_object* x_2; 
x_2 = lean_box(0);
return x_2;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_identityPatch___lam__0___boxed(lean_object* x_1) {
_start:
{
lean_object* x_2; 
x_2 = lp_suture_x2dproofs_Suture_identityPatch___lam__0(x_1);
lean_dec_ref(x_1);
return x_2;
}
}
LEAN_EXPORT uint8_t lp_suture_x2dproofs_Suture_actualTouchSet___lam__0(lean_object* x_1, lean_object* x_2, lean_object* x_3) {
_start:
{
lean_object* x_4; lean_object* x_5; lean_object* x_6; uint8_t x_7; 
x_4 = lean_alloc_closure((void*)(l_instDecidableEqString___boxed), 2, 0);
lean_inc_ref(x_3);
lean_inc_ref(x_2);
x_5 = lp_suture_x2dproofs_Suture_StaticPatch_apply(x_1, x_2, x_3);
x_6 = lean_apply_1(x_2, x_3);
x_7 = l_Option_instDecidableEq___redArg(x_4, x_5, x_6);
if (x_7 == 0)
{
uint8_t x_8; 
x_8 = 1;
return x_8;
}
else
{
uint8_t x_9; 
x_9 = 0;
return x_9;
}
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_actualTouchSet___lam__0___boxed(lean_object* x_1, lean_object* x_2, lean_object* x_3) {
_start:
{
uint8_t x_4; lean_object* x_5; 
x_4 = lp_suture_x2dproofs_Suture_actualTouchSet___lam__0(x_1, x_2, x_3);
x_5 = lean_box(x_4);
return x_5;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_actualTouchSet(lean_object* x_1, lean_object* x_2) {
_start:
{
lean_object* x_3; lean_object* x_4; lean_object* x_5; 
x_3 = lean_ctor_get(x_1, 0);
lean_inc(x_3);
x_4 = lean_alloc_closure((void*)(lp_suture_x2dproofs_Suture_actualTouchSet___lam__0___boxed), 3, 2);
lean_closure_set(x_4, 0, x_1);
lean_closure_set(x_4, 1, x_2);
x_5 = lp_mathlib_Multiset_filter___redArg(x_4, x_3);
return x_5;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_compose___lam__0(lean_object* x_1, lean_object* x_2, lean_object* x_3, lean_object* x_4, lean_object* x_5, lean_object* x_6) {
_start:
{
uint8_t x_7; 
lean_inc_ref(x_6);
lean_inc_ref(x_1);
x_7 = lp_mathlib_Multiset_decidableMem___aux__1___redArg(x_1, x_6, x_2);
if (x_7 == 0)
{
uint8_t x_8; 
lean_dec_ref(x_5);
lean_inc_ref(x_6);
x_8 = lp_mathlib_Multiset_decidableMem___aux__1___redArg(x_1, x_6, x_3);
if (x_8 == 0)
{
lean_object* x_9; 
lean_dec_ref(x_6);
lean_dec_ref(x_4);
x_9 = lean_box(0);
return x_9;
}
else
{
lean_object* x_10; 
x_10 = lean_apply_1(x_4, x_6);
return x_10;
}
}
else
{
lean_object* x_11; 
lean_dec_ref(x_4);
lean_dec(x_3);
lean_dec_ref(x_1);
x_11 = lean_apply_1(x_5, x_6);
return x_11;
}
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1(lean_object* x_1, size_t x_2, size_t x_3, lean_object* x_4) {
_start:
{
uint8_t x_5; 
x_5 = lean_usize_dec_eq(x_2, x_3);
if (x_5 == 0)
{
size_t x_6; size_t x_7; lean_object* x_8; uint8_t x_9; 
x_6 = 1;
x_7 = lean_usize_sub(x_2, x_6);
x_8 = lean_array_uget_borrowed(x_1, x_7);
x_9 = lp_mathlib_List_elem___at___00Mathlib_Meta_FunProp_logError_spec__0(x_8, x_4);
if (x_9 == 0)
{
lean_object* x_10; 
lean_inc(x_8);
x_10 = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(x_10, 0, x_8);
lean_ctor_set(x_10, 1, x_4);
x_2 = x_7;
x_4 = x_10;
goto _start;
}
else
{
x_2 = x_7;
goto _start;
}
}
else
{
return x_4;
}
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1___boxed(lean_object* x_1, lean_object* x_2, lean_object* x_3, lean_object* x_4) {
_start:
{
size_t x_5; size_t x_6; lean_object* x_7; 
x_5 = lean_unbox_usize(x_2);
lean_dec(x_2);
x_6 = lean_unbox_usize(x_3);
lean_dec(x_3);
x_7 = lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1(x_1, x_5, x_6, x_4);
lean_dec_ref(x_1);
return x_7;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0(lean_object* x_1, lean_object* x_2) {
_start:
{
lean_object* x_3; lean_object* x_4; lean_object* x_5; uint8_t x_6; 
x_3 = lean_array_mk(x_2);
x_4 = lean_array_get_size(x_3);
x_5 = lean_unsigned_to_nat(0u);
x_6 = lean_nat_dec_lt(x_5, x_4);
if (x_6 == 0)
{
lean_dec_ref(x_3);
return x_1;
}
else
{
size_t x_7; size_t x_8; lean_object* x_9; 
x_7 = lean_usize_of_nat(x_4);
x_8 = 0;
x_9 = lp_suture_x2dproofs___private_Init_Data_Array_Basic_0__Array_foldrMUnsafe_fold___at___00List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0_spec__1(x_3, x_7, x_8, x_1);
lean_dec_ref(x_3);
return x_9;
}
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Multiset_ndunion___at___00Suture_compose_spec__0(lean_object* x_1, lean_object* x_2) {
_start:
{
lean_object* x_3; 
x_3 = lp_suture_x2dproofs_List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0(x_2, x_1);
return x_3;
}
}
LEAN_EXPORT lean_object* lp_suture_x2dproofs_Suture_compose(lean_object* x_1, lean_object* x_2) {
_start:
{
lean_object* x_3; lean_object* x_4; uint8_t x_5; 
x_3 = lean_ctor_get(x_1, 0);
lean_inc(x_3);
x_4 = lean_ctor_get(x_1, 1);
lean_inc_ref(x_4);
lean_dec_ref(x_1);
x_5 = !lean_is_exclusive(x_2);
if (x_5 == 0)
{
lean_object* x_6; lean_object* x_7; lean_object* x_8; lean_object* x_9; lean_object* x_10; 
x_6 = lean_ctor_get(x_2, 0);
x_7 = lean_ctor_get(x_2, 1);
x_8 = lean_alloc_closure((void*)(l_instDecidableEqString___boxed), 2, 0);
lean_inc(x_3);
lean_inc(x_6);
x_9 = lean_alloc_closure((void*)(lp_suture_x2dproofs_Suture_compose___lam__0), 6, 5);
lean_closure_set(x_9, 0, x_8);
lean_closure_set(x_9, 1, x_6);
lean_closure_set(x_9, 2, x_3);
lean_closure_set(x_9, 3, x_4);
lean_closure_set(x_9, 4, x_7);
x_10 = lp_suture_x2dproofs_List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0(x_6, x_3);
lean_ctor_set(x_2, 1, x_9);
lean_ctor_set(x_2, 0, x_10);
return x_2;
}
else
{
lean_object* x_11; lean_object* x_12; lean_object* x_13; lean_object* x_14; lean_object* x_15; lean_object* x_16; 
x_11 = lean_ctor_get(x_2, 0);
x_12 = lean_ctor_get(x_2, 1);
lean_inc(x_12);
lean_inc(x_11);
lean_dec(x_2);
x_13 = lean_alloc_closure((void*)(l_instDecidableEqString___boxed), 2, 0);
lean_inc(x_3);
lean_inc(x_11);
x_14 = lean_alloc_closure((void*)(lp_suture_x2dproofs_Suture_compose___lam__0), 6, 5);
lean_closure_set(x_14, 0, x_13);
lean_closure_set(x_14, 1, x_11);
lean_closure_set(x_14, 2, x_3);
lean_closure_set(x_14, 3, x_4);
lean_closure_set(x_14, 4, x_12);
x_15 = lp_suture_x2dproofs_List_foldrTR___at___00Multiset_ndunion___at___00Suture_compose_spec__0_spec__0(x_11, x_3);
x_16 = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(x_16, 0, x_15);
lean_ctor_set(x_16, 1, x_14);
return x_16;
}
}
}
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_mathlib_Mathlib_Data_Finset_Basic(uint8_t builtin);
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_suture_x2dproofs_SutureProofs_Foundations(uint8_t builtin) {
lean_object * res;
if (_G_initialized) return lean_io_result_mk_ok(lean_box(0));
_G_initialized = true;
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_mathlib_Mathlib_Data_Finset_Basic(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
return lean_io_result_mk_ok(lean_box(0));
}
#ifdef __cplusplus
}
#endif
