#[tokio::main]
async fn main(){
    qexed_log::log_init().await;
    log::info!("test");
    tokio::spawn(qexed_update_check::check_version());
    
    loop {
        
    }
}