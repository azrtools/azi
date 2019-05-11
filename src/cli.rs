use std::env::args_os;
use std::error::Error;
use std::io::stdin;
use std::io::Read;
use std::slice::Iter;

use env_logger;
use log::LevelFilter;

use crate::client::Client;
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
const OUTPUT: Flag = (
    "-o, --output <format>",
    "Set output format, one of 'text' (default) or 'json'",
    true,
);

const GLOBAL_FLAGS: &[Flag] = &[HELP, VERSION, DEBUG, TRACE, TENANT, OUTPUT];

const LIST: Command = (
    "list",
    "List existing resource groups",
    &[HELP, LIST_RESOURCES],
);
const LIST_RESOURCES: Flag = ("-r, --resources", "Also list all resources", false);

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

const GET: Command = ("get", "Execute a GET request", &[HELP, REQUEST]);
const POST: Command = ("post", "Execute a POST request", &[HELP, REQUEST]);
const REQUEST: Flag = ("<request>", "The request to execute", false);

const COMMANDS: &[Command] = &[LIST, DOMAINS, DNS, IP, COSTS, GET, POST];

const MAX_COLUMN: usize = 80;

macro_rules! parse_error {
    ($($arg:tt)*) => (Box::<Error>::from(ParseError(format!($($arg)*))))
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
    };
    logger.init();

    let output: &Output = match args.get_global_flag_arg(&OUTPUT) {
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
        let service = Service::new(client);

        let context = Context { service: &service };

        match command {
            LIST => {
                let list_resources = args.has_command_flag(&LIST_RESOURCES);
                let result = list(&context, list_resources)?;
                output.print_list_results(&result)?;
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
                    } else {
                        return Err(Box::from("invalid length!"));
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
                let mut buffer = String::new();
                stdin().read_to_string(&mut buffer)?;
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

        fn parse_flag(flags: &[Flag], arg: &str, it: &mut Iter<&str>) -> Result<Arg> {
            let found = flags
                .iter()
                .find(|flag| arg == short_flag(flag) || arg == long_flag(flag));
            if let Some(flag) = found {
                if flag.2 {
                    if let Some(&arg) = it.next() {
                        return Ok((*flag, arg.to_string()));
                    } else {
                        return Err(parse_error!("missing argument for {}", long_flag(flag)));
                    }
                } else {
                    return Ok((*flag, "".to_owned()));
                }
            } else {
                return Err(parse_error!("unknown option: {}", arg));
            }
        }

        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            if double_dash {
                command_args.push(arg.to_string());
            } else if arg == "--" {
                double_dash = true;
            } else if let Some(command) = command {
                if arg.starts_with("-") {
                    command_flags.push(parse_flag(command.2, arg, &mut it)?);
                } else {
                    command_args.push(arg.to_string());
                }
            } else {
                if arg.starts_with("-") {
                    global_flags.push(parse_flag(GLOBAL_FLAGS, arg, &mut it)?);
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
            self.print_prefix(&format!("  {0:1$}    ", flag.0, max_len));
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
