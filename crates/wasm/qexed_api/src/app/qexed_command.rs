
// // qexed
// use serde::ser::SerializeSeq;
// use serde::{Deserialize, Deserializer, Serialize, Serializer};
// pub trait Command {
//     const NAME:&'static str;
// }

// // help [字符串]
// // help [整数]
// pub struct HelpCommand{
//     pub page_or_cmd:SubHelpCommand,    
// }
// pub struct H2{}
// impl Command for H2 {
//     const NAME:&'static str ="123123";
// }
// impl Command for HelpCommand {
//     const NAME:&'static str ="help";
// }
// pub enum SubHelpCommand{
//     Page(i32), // 对应 /help [整数]
//     Cmd(String),// 对应 /help [字符串]
//     Cmds(Vec<String>), // 对应 /help [字符串] [字符串] [字符串](注:无数个参数)
//     Cmds2((i32,String)), // 对应 /help [整数] [字符串]
//     Cmds3((i32,[String;3],i32)), // 对应 /help [整数] [字符串] [字符串] [字符串] [字符串] [整数]
//     Other(H2),// 对应 /help 123123
// }
// fn b(){

// }
// fn c(value:String){

// }
// fn d(value:i32){

// }
// fn a(){
//     Command("qexed")
//         .SubCommand(
//             Command("about")
//                 .Function::<()>(b)
//         ).Function::<String>(
//             c
//         ).Function::<i32>(
//             d
//         )
// }