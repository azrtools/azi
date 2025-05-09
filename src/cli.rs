use std::env::args_os;
use std::error::Error;
use std::io::stdin;
use std::io::Read;
use std::io::Write;
use std::slice::Iter;

use env_logger;
use log::LevelFilter;

use crate::client::Client;
use crate::commands::clusters;
use crate::commands::costs;
use crate::commands::dns;
use crate::commands::domains;
use crate::commands::get;
use crate::commands::ip;
use crate::commands::list;
use crate::commands::post;
use crate::commands::Context;
use crate::error::AppError;
use crate::error::AppError::ParseError;
use crate::output::JsonOutput;
use crate::output::Output;
use crate::output::TextOutput;
use crate::service::Filter;
use crate::service::Service;
use crate::service::Timeframe;
use crate::utils::convert_str;
use crate::utils::days_of_month;
use crate::utils::Result;

type Flag = (&'static str, &'static str, bool);

type Command = (&'static str, &'static str, &'static [Flag]);

const HELP: Flag = ("-h, --help", "Show this help message and exit", false);
const VERSION: Flag = ("--version", "Show program's version number and exit", false);
const DEBUG: Flag = ("--debug", "Show debugging output", false);
const TRACE: Flag = ("--trace", "Show even more debugging output", false);
const TENANT: Flag = (
    "-t, --tenant <tenant>",
    "Set the Active Directory tenant to use",
    true,
);
const FILTER: Flag = (
    "-f, --filter <filter>",
    "Filter subscriptions to display",
    true,
);
const OUTPUT: Flag = (
    "-o, --output <format>",
    "Set output format, one of 'text' (default) or 'json'",
    true,
);

const GLOBAL_FLAGS: &[Flag] = &[HELP, VERSION, DEBUG, TRACE, TENANT, FILTER, OUTPUT];

const LIST: Command = (
    "list",
    "Show resource groups and resources",
    &[HELP, LIST_ID, LIST_RESOURCES, LIST_FILTER],
);
const LIST_ID: Flag = ("--id", "Also display resource IDs", false);
const LIST_RESOURCES: Flag = ("-r, --resources", "Also list all resources", false);
const LIST_FILTER: Flag = ("[<filter>]", "Filter resources by name", false);

const CLUSTERS: Command = (
    "clusters",
    "Show Kubernetes clusters",
    &[
        HELP,
        CLUSTERS_ID,
        CLUSTERS_AGENT_POOLS,
        CLUSTERS_RESOURCES,
        CLUSTERS_ALL_RESOURCES,
        CLUSTERS_CONTAINERS,
        CLUSTERS_FILTER,
    ],
);
const CLUSTERS_ID: Flag = ("--id", "Also display resource IDs", false);
const CLUSTERS_AGENT_POOLS: Flag = ("-p, --pools", "List agent pools", false);
const CLUSTERS_RESOURCES: Flag = ("-r, --resources", "List Kubernetes resources", false);
const CLUSTERS_ALL_RESOURCES: Flag = (
    "-R, --all-resources",
    "All resources, including Kubernetes system resources",
    false,
);
const CLUSTERS_CONTAINERS: Flag = (
    "-c, --containers",
    "List deployment container templates",
    false,
);
const CLUSTERS_FILTER: Flag = ("[<filter>]", "Filter clusters by name", false);

const DOMAINS: Command = (
    "domains",
    "Show all domains and hosting resource groups",
    &[HELP, DOMAIN],
);
const DOMAIN: Flag = (
    "[<domain>]",
    "The domain to filter for, otherwise all domains are shown",
    false,
);

const DNS: Command = ("dns", "Show DNS records and mapped IP addresses", &[HELP]);

const IP: Command = ("ip", "Show currently used IP addresses", &[HELP]);

const COSTS: Command = ("costs", "Show the current resource costs", &[HELP, PERIOD]);
const PERIOD: Flag = (
    "[<period>]",
    "The billing period to show costs for, for example 2019 or 201905. By default, the costs for the current month are shown",
    false,
);

const GET: Command = ("get", "Execute HTTP GET request", &[HELP, REQUEST]);
const POST: Command = ("post", "Execute HTTP POST request", &[HELP, BODY, REQUEST]);
const BODY: Flag = (
    "-d, --data <data>",
    "The POST data, or - to read from stdin",
    true,
);
const REQUEST: Flag = ("<request>", "The request to execute", false);

const COMMANDS: &[Command] = &[LIST, CLUSTERS, DOMAINS, DNS, IP, COSTS, GET, POST];

const MAX_COLUMN: usize = 80;

const PROGRAM_VERSION: &'static str = env!("CARGO_PKG_VERSION");

macro_rules! parse_error {
    ($($arg:tt)*) => (Box::<dyn Error>::from(ParseError(format!($($arg)*))))
}

pub fn run() {
    let str_args: Vec<String> = args_os().skip(1).map(convert_str).collect();

    let args = match Args::parse(str_args.iter().map(AsRef::as_ref).collect()) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {}", err);
            Printer::new().print_usage();
            return;
        }
    };

    if args.has_global_flag(&HELP) {
        Printer::new().print_help();
        return;
    }

    if args.has_global_flag(&VERSION) {
        Printer::new().print_version();
        return;
    }

    let command = match args.command() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {}", err);
            Printer::new().print_usage();
            return;
        }
    };

    if args.has_command_flag(&HELP) {
        Printer::new().print_command_help(&command);
        return;
    }

    let mut logger = env_logger::Builder::new();
    if args.has_global_flag(&TRACE) {
        logger.filter(Some("azi"), LevelFilter::Trace);
    } else if args.has_global_flag(&DEBUG) {
        logger.filter(Some("azi"), LevelFilter::Debug);
    } else {
        logger.filter(Some("azi"), LevelFilter::Info);
        logger.format(|buf, record| writeln!(buf, "[{}] {}", record.level(), record.args()));
    };
    logger.init();

    let output: &dyn Output = match args.get_global_flag_arg(&OUTPUT) {
        Some("json") => &JsonOutput {},
        Some("text") | None => &TextOutput {},
        Some(arg) => {
            eprintln!("error: unknown output format: {}", arg);
            Printer::new().print_usage();
            return;
        }
    };

    let run_command = || -> Result<()> {
        let client = Client::new(args.get_global_flag_arg(&TENANT))?;
        let service = Service::new(client, Filter::new(args.get_global_flag_arg(&FILTER)));

        let context = Context { service: &service };

        match command {
            LIST => {
                let id = args.has_command_flag(&LIST_ID);
                let list_resources = args.has_command_flag(&LIST_RESOURCES);
                let result = list(&context, list_resources, args.get_arg_opt(0))?;
                output.print_list_results(&result, id)?;
            }
            CLUSTERS => {
                let id = args.has_command_flag(&CLUSTERS_ID);
                let pools = args.has_command_flag(&CLUSTERS_AGENT_POOLS);
                let resources = args.has_command_flag(&CLUSTERS_RESOURCES);
                let all_resources = args.has_command_flag(&CLUSTERS_ALL_RESOURCES);
                let containers = args.has_command_flag(&CLUSTERS_CONTAINERS);
                let result = clusters(
                    &context,
                    pools,
                    resources || all_resources || containers,
                    all_resources,
                    containers,
                    args.get_arg_opt(0),
                )?;
                output.print_clusters(&result, id, resources || all_resources)?;
            }
            DOMAINS => {
                let result = domains(&context, args.get_arg_opt(0))?;
                output.print_domains(&result)?;
            }
            DNS => {
                let result = dns(&context)?;
                output.print_dns_results(&result)?;
            }
            IP => {
                let result = ip(&context)?;
                output.print_ip_results(&result)?;
            }
            COSTS => {
                fn parse_period(period: &str) -> Result<Timeframe> {
                    if period.len() == 4 {
                        let year: u32 = period.parse()?;
                        return Ok(Timeframe::Custom {
                            from: format!("{:04}-01-01", year),
                            to: format!("{:04}-12-31", year),
                        });
                    } else if period.len() == 6 {
                        let year: u32 = period[0..4].parse()?;
                        let month: u32 = period[4..6].parse()?;
                        let days = days_of_month(year, month)?;
                        return Ok(Timeframe::Custom {
                            from: format!("{:04}-{:02}-01", year, month),
                            to: format!("{:04}-{:02}-{:02}", year, month, days),
                        });
                    } else if period.len() == 8 {
                        let year: u32 = period[0..4].parse()?;
                        let month: u32 = period[4..6].parse()?;
                        let day: u32 = period[6..8].parse()?;
                        return Ok(Timeframe::Custom {
                            from: format!("{:04}-{:02}-{:02}", year, month, day),
                            to: format!("{:04}-{:02}-{:02}", year, month, day),
                        });
                    } else if period.len() == 13 && &period[6..7] == "-" {
                        let from_year: u32 = period[0..4].parse()?;
                        let from_month: u32 = period[4..6].parse()?;
                        let to_year: u32 = period[7..11].parse()?;
                        let to_month: u32 = period[11..13].parse()?;
                        let to_days = days_of_month(to_year, to_month)?;
                        return Ok(Timeframe::Custom {
                            from: format!("{:04}-{:02}-01", from_year, from_month),
                            to: format!("{:04}-{:02}-{:02}", to_year, to_month, to_days),
                        });
                    } else {
                        return Err(Box::from("invalid period!"));
                    }
                }
                let result = match args.get_arg_opt(0) {
                    Some(period) => {
                        let timeframe = parse_period(period)
                            .or(Err(parse_error!("invalid period: {}", period)))?;
                        costs(&context, &timeframe)?
                    }
                    None => costs(&context, &Timeframe::MonthToDate)?,
                };
                output.print_cost_results(&result)?;
            }
            GET => {
                let request = args.get_arg(0, &REQUEST)?;
                let result = get(&context, request)?;
                output.print_value(&result)?;
            }
            POST => {
                let request = args.get_arg(0, &REQUEST)?;
                let body = args.get_command_flag_arg(&BODY);
                let buffer = if body.is_some() && body.unwrap() == "-" {
                    let mut buffer = String::new();
                    stdin().read_to_string(&mut buffer)?;
                    buffer
                } else {
                    body.unwrap_or("").to_owned()
                };
                let result = post(&context, request, &buffer)?;
                output.print_value(&result)?;
            }
            _ => return Err(parse_error!("unknown command!")),
        }
        return Ok(());
    };

    match run_command() {
        Ok(_) => (),
        Err(err) => {
            eprintln!("error: {}", err);
            if let Ok(app_err) = err.downcast::<AppError>() {
                if let ParseError(_) = *app_err {
                    Printer::new().print_command_usage(&command);
                }
            }
        }
    }
}

fn short_flag(flag: &Flag) -> &str {
    return match flag.0.find(",") {
        Some(pos) => &flag.0[..pos],
        None => "",
    };
}

fn long_flag(flag: &Flag) -> &str {
    return match flag.0.find(",") {
        Some(pos) => &flag.0[pos + 2..],
        None => flag.0,
    };
}

#[derive(Debug)]
struct Args {
    global_flags: Vec<Arg>,
    command: Option<Command>,
    command_flags: Vec<Arg>,
    command_args: Vec<String>,
}

type Arg = (Flag, String);

impl Args {
    fn parse(args: Vec<&str>) -> Result<Args> {
        let mut command: Option<Command> = None;
        let mut global_flags = Vec::new();
        let mut command_flags = Vec::new();
        let mut command_args = Vec::new();

        let mut double_dash = false;

        fn parse_long_flag(
            flags: &[Flag],
            arg: &str,
            it: &mut Iter<&str>,
            target: &mut Vec<Arg>,
        ) -> Result<()> {
            let found = flags.iter().find(|flag| arg == long_flag(flag));
            if let Some(flag) = found {
                if flag.2 {
                    if let Some(&arg) = it.next() {
                        target.push((*flag, arg.to_owned()));
                    } else {
                        return Err(parse_error!("missing argument for {}", long_flag(flag)));
                    }
                } else {
                    target.push((*flag, "".to_owned()));
                }
                return Ok(());
            } else {
                return Err(parse_error!("unknown option: {}", arg));
            }
        }

        fn parse_short_flags(
            flags: &[Flag],
            arg: &str,
            it: &mut Iter<&str>,
            target: &mut Vec<Arg>,
        ) -> Result<()> {
            for i in 1..arg.len() {
                let a = format!("-{}", &arg[i..i + 1]);
                let found = flags.iter().find(|flag| a == short_flag(flag));
                if let Some(flag) = found {
                    if flag.2 {
                        if i + 1 < arg.len() {
                            target.push((*flag, arg[i + 1..].to_owned()));
                            break;
                        } else if let Some(&arg) = it.next() {
                            target.push((*flag, arg.to_owned()));
                        } else {
                            return Err(parse_error!("missing argument for {}", long_flag(flag)));
                        }
                    } else {
                        target.push((*flag, "".to_owned()));
                    }
                } else {
                    return Err(parse_error!("unknown option: {}", arg));
                }
            }
            return Ok(());
        }

        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            if double_dash {
                command_args.push(arg.to_owned());
            } else if arg == "--" {
                double_dash = true;
            } else if let Some(command) = command {
                if arg.starts_with("--") {
                    parse_long_flag(command.2, arg, &mut it, &mut command_flags)?;
                } else if arg.starts_with("-") && arg.len() > 1 {
                    parse_short_flags(command.2, arg, &mut it, &mut command_flags)?;
                } else {
                    command_args.push(arg.to_owned());
                }
            } else {
                if arg.starts_with("--") {
                    parse_long_flag(GLOBAL_FLAGS, arg, &mut it, &mut global_flags)?;
                } else if arg.starts_with("-") && arg.len() > 1 {
                    parse_short_flags(GLOBAL_FLAGS, arg, &mut it, &mut global_flags)?;
                } else {
                    let found = COMMANDS.iter().find(|command| arg == command.0);
                    if let Some(cmd) = found {
                        command = Some(*cmd);
                    } else {
                        return Err(parse_error!("unknown command: {}", arg));
                    }
                }
            }
        }

        return Ok(Args {
            global_flags,
            command,
            command_flags,
            command_args,
        });
    }

    fn command(&self) -> Result<Command> {
        return self.command.ok_or(parse_error!("command missing!"));
    }

    fn has_global_flag(&self, flag: &Flag) -> bool {
        for global_flag in &self.global_flags {
            if &global_flag.0 == flag {
                return true;
            }
        }
        return false;
    }

    fn has_command_flag(&self, flag: &Flag) -> bool {
        for command_flag in &self.command_flags {
            if &command_flag.0 == flag {
                return true;
            }
        }
        return false;
    }

    fn get_global_flag_arg(&self, flag: &Flag) -> Option<&str> {
        for global_flag in &self.global_flags {
            if &global_flag.0 == flag {
                return Some(&global_flag.1);
            }
        }
        return None;
    }

    fn get_command_flag_arg(&self, flag: &Flag) -> Option<&str> {
        for command_flag in &self.command_flags {
            if &command_flag.0 == flag {
                return Some(&command_flag.1);
            }
        }
        return None;
    }

    fn get_arg(&self, index: usize, flag: &Flag) -> Result<&String> {
        return self
            .command_args
            .get(index)
            .ok_or(parse_error!("missing argument: {}", flag.0));
    }

    fn get_arg_opt(&self, index: usize) -> Option<&String> {
        return self.command_args.get(index);
    }
}

struct Printer {
    column: usize,
    indent: usize,
}

impl Printer {
    fn new() -> Printer {
        return Printer {
            column: 0,
            indent: 0,
        };
    }

    fn print_help(&mut self) {
        self.print_usage();

        self.println();
        self.print_description("Show Azure information.");

        self.println();
        self.print_options(GLOBAL_FLAGS);

        self.println();
        self.print_commands(COMMANDS);
    }

    fn print_version(&mut self) {
        self.print(PROGRAM_VERSION);
        self.println();
    }

    fn print_usage(&mut self) {
        self.print_prefix("usage: azi");
        self.print_flags(GLOBAL_FLAGS);
        self.print(" <command>");
        self.print(" [<args>]");
        self.println();
    }

    fn print_command_help(&mut self, command: &Command) {
        self.print_command_usage(command);

        self.println();
        self.print_description(command.1);

        self.println();
        self.print_options(command.2);
    }

    fn print_command_usage(&mut self, command: &Command) {
        self.print_prefix(&format!("usage: azi {}", command.0));
        self.print_flags(command.2);
        self.println();
    }

    fn print_flags(&mut self, flags: &[Flag]) {
        for flag in flags {
            if flag.0.starts_with("-") {
                if short_flag(flag).is_empty() {
                    self.print(&[" [", long_flag(flag), "]"].join(""));
                } else {
                    self.print(&[" [", short_flag(flag), "]"].join(""));
                }
            } else {
                self.print(&[" ", flag.0].join(""));
            }
        }
    }

    fn print_description(&mut self, message: &str) {
        self.print_text(message);
    }

    fn print_options(&mut self, flags: &[Flag]) {
        if flags.is_empty() {
            return;
        }

        eprintln!("Options:");

        let mut max_len = 0;
        for flag in flags {
            if flag.0.len() > max_len {
                max_len = flag.0.len();
            }
        }

        for flag in flags {
            if flag.0.starts_with("[") {
                self.print_prefix(&format!(
                    "  {0:1$}    ",
                    &flag.0[1..flag.0.len() - 1],
                    max_len
                ));
            } else {
                self.print_prefix(&format!("  {0:1$}    ", flag.0, max_len));
            }
            self.print_text(flag.1);
        }
    }

    fn print_commands(&mut self, commands: &[Command]) {
        eprintln!("Commands:");

        let mut max_len = 0;
        for command in commands {
            if command.0.len() > max_len {
                max_len = command.0.len();
            }
        }

        for command in commands {
            eprintln!("  {0:1$}    {2}", command.0, max_len, command.1);
        }
    }

    fn print_prefix(&mut self, message: &str) {
        self.column = 0;
        self.print(message);
        self.indent = self.column;
    }

    fn print_text(&mut self, message: &str) {
        for m in message.split(" ") {
            self.print(m);
            self.print(" ");
        }
        self.println();
    }

    fn print(&mut self, message: &str) {
        if self.column + message.len() > MAX_COLUMN {
            eprintln!();
            eprint!("{0:1$}{2}", "", self.indent, message);
            self.column = self.indent + message.len();
        } else {
            eprint!("{}", message);
            self.column += message.len();
        }
    }

    fn println(&mut self) {
        eprintln!();
        self.column = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::long_flag;
    use super::short_flag;
    use super::Args;
    use super::DEBUG;
    use super::GET;
    use super::HELP;

    #[test]
    fn test_short_flag() {
        assert_eq!("-h", short_flag(&HELP));
    }

    #[test]
    fn test_long_flag() {
        assert_eq!("--help", long_flag(&HELP));
    }

    #[test]
    fn test_parse() {
        let args = Args::parse(vec!["--debug", "get", "test", "--"]).unwrap();
        assert_eq!(vec!((DEBUG, "".to_owned())), args.global_flags);
        assert_eq!(Some(GET), args.command);
        assert_eq!(0, args.command_flags.len());
        assert_eq!(vec!("test"), args.command_args);
    }

    #[test]
    fn test_parse_missing_command() {
        assert_eq!(None, Args::parse(vec!("--debug")).unwrap().command);
    }
}
