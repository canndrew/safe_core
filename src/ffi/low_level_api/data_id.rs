// Copyright 2016 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net
// Commercial License, version 1.0 or later, or (2) The General Public License
// (GPL), version 3, depending on which licence you accepted on initial access
// to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project
// generally, you agree to be bound by the terms of the MaidSafe Contributor
// Agreement, version 1.0.
// This, along with the Licenses can be found in the root directory of this
// project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network
// Software distributed under the GPL Licence is distributed on an "AS IS"
// BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
// implied.
//
// Please review the Licences for the specific language governing permissions
// and limitations relating to use of the SAFE Network Software.

use ffi::{OpaqueCtx, Session, helper};
use ffi::object_cache::DataIdHandle;
use libc::int32_t;
use routing::{DataIdentifier, XOR_NAME_LEN, XorName};
use std::ptr;

/// Construct DataIdentifier for StructuredData.
#[no_mangle]
pub unsafe extern "C" fn data_id_new_struct_data(session: *const Session,
                                                 user_data: OpaqueCtx,
                                                 type_tag: u64,
                                                 id: *const [u8; XOR_NAME_LEN],
                                                 o_cb: extern "C" fn(int32_t,
                                                                     OpaqueCtx,
                                                                     DataIdHandle))
                                                 -> i32 {
    helper::catch_unwind_i32(|| {
        let xor_id = XorName(*id);
        let data_id = DataIdentifier::Structured(xor_id, type_tag);
        let handle = unwrap!((*session).object_cache()).insert_data_id(data_id);
        o_cb(0, user_data, handle);
        0
    })
}

/// Construct DataIdentifier for ImmutableData.
#[no_mangle]
pub unsafe extern "C" fn data_id_new_immut_data(session: *const Session,
                                                id: *const [u8; XOR_NAME_LEN],
                                                o_handle: *mut DataIdHandle)
                                                -> i32 {
    helper::catch_unwind_i32(|| {
        let xor_id = XorName(*id);
        let data_id = DataIdentifier::Immutable(xor_id);
        let handle = unwrap!((*session).object_cache()).insert_data_id(data_id);
        ptr::write(o_handle, handle);
        0
    })
}

/// Construct DataIdentifier for AppendableData.
#[no_mangle]
pub unsafe extern "C" fn data_id_new_appendable_data(session: *const Session,
                                                     id: *const [u8; XOR_NAME_LEN],
                                                     is_private: bool,
                                                     o_handle: *mut DataIdHandle)
                                                     -> i32 {
    helper::catch_unwind_i32(|| {
        let xor_id = XorName(*id);
        let data_id = if is_private {
            DataIdentifier::PrivAppendable(xor_id)
        } else {
            DataIdentifier::PubAppendable(xor_id)
        };
        let handle = unwrap!((*session).object_cache()).insert_data_id(data_id);
        ptr::write(o_handle, handle);
        0
    })
}

/// Free DataIdentifier handle
#[no_mangle]
pub unsafe extern "C" fn data_id_free(session: *const Session, handle: DataIdHandle) -> i32 {
    helper::catch_unwind_i32(|| {
        let _ = ffi_try!(unwrap!((*session).object_cache()).remove_data_id(handle));
        0
    })
}

#[cfg(test)]
mod tests {
    use ffi::{FfiError, OpaqueCtx, test_utils};
    use rand;
    use routing::XOR_NAME_LEN;
    use std::ptr;
    use super::*;

    #[test]
    fn create_and_free() {
        let sess = test_utils::create_session();
        let sess_ptr = Box::into_raw(Box::new(sess.clone()));

        let type_tag = rand::random();
        let struct_id_arr: [u8; XOR_NAME_LEN] = rand::random();

        let immut_id_arr: [u8; XOR_NAME_LEN] = rand::random();

        let priv_app_id_arr: [u8; XOR_NAME_LEN] = rand::random();
        let pub_app_id_arr: [u8; XOR_NAME_LEN] = rand::random();

        let mut data_id_handle_immut = 0;
        let data_id_handle_struct = 0;
        let mut data_id_handle_priv_appendable = 0;
        let mut data_id_handle_pub_appendable = 0;

        unsafe {
            assert_eq!(data_id_new_struct_data(sess_ptr,
                                               OpaqueCtx(ptr::null_mut()),
                                               type_tag,
                                               &struct_id_arr,
                                               handle_struct_data),
                       0);

            extern "C" fn handle_struct_data(errcode: i32, _: OpaqueCtx, _: u64) {
                assert_eq!(errcode, 0);
            }

            assert_eq!(data_id_new_immut_data(sess_ptr, &immut_id_arr, &mut data_id_handle_immut),
                       0);
            assert_eq!(data_id_new_appendable_data(sess_ptr,
                                                   &priv_app_id_arr,
                                                   true,
                                                   &mut data_id_handle_priv_appendable),
                       0);
            assert_eq!(data_id_new_appendable_data(sess_ptr,
                                                   &pub_app_id_arr,
                                                   false,
                                                   &mut data_id_handle_pub_appendable),
                       0);
        }

        {
            let mut obj_cache = unwrap!(sess.object_cache());
            let _ = unwrap!(obj_cache.get_data_id(data_id_handle_struct));
            let _ = unwrap!(obj_cache.get_data_id(data_id_handle_immut));
            let _ = unwrap!(obj_cache.get_data_id(data_id_handle_priv_appendable));
            let _ = unwrap!(obj_cache.get_data_id(data_id_handle_pub_appendable));
        }

        unsafe {
            assert_eq!(data_id_free(sess_ptr, data_id_handle_struct), 0);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_immut), 0);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_priv_appendable), 0);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_pub_appendable), 0);
        }

        let err_code = FfiError::InvalidDataIdHandle.into();
        unsafe {
            assert_eq!(data_id_free(sess_ptr, data_id_handle_struct), err_code);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_immut), err_code);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_priv_appendable),
                       err_code);
            assert_eq!(data_id_free(sess_ptr, data_id_handle_pub_appendable),
                       err_code);
        }

        {
            let mut obj_cache = unwrap!(sess.object_cache());
            assert!(obj_cache.get_data_id(data_id_handle_struct).is_err());
            assert!(obj_cache.get_data_id(data_id_handle_immut).is_err());
            assert!(obj_cache.get_data_id(data_id_handle_priv_appendable).is_err());
            assert!(obj_cache.get_data_id(data_id_handle_pub_appendable).is_err());
        }
    }
}
