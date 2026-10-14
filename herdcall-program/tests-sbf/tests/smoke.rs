use herdcall_sbf_tests::env::*;
use litesvm::LiteSVM;

#[test]
fn programs_load() {
    let mut svm = LiteSVM::new();
    load_programs(&mut svm);
    for id in [HERDCALL_ID, DAMM_ID] {
        let a = svm.get_account(&id).expect("program account");
        assert!(a.executable);
        println!("REPORT program {id} owner {} len {}", a.owner, a.data.len());
    }
}
