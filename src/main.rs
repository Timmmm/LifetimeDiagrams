// Read in a Rust snippet, convert comments of lifetimes to a nice SVG.
//
//
//
/* Example input:

{
	let r;         // -------\ Lifetime of `r`

	{
		let x = 5; // -\ Lifetime of `b`
		r = &x;
	}              // -/

	println!("r: {}", r);

				   // -------/
}

*/

#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate svg;
extern crate markdown;

use std::io::{self, Read};
use regex::Regex;
use std::collections::HashMap;

mod escape;

use escape::*;

// Finds the lifetime annotations and removes the annotations from the code.
fn find_lifetimes(code: &mut Vec<String>) -> Vec<Lifetime> {

	lazy_static! {
		static ref RE: Regex = Regex::new(r"^(.*)// (-+)([/\\]) ?(.*)$").unwrap();
	}

	let mut unfinished_lifetimes: HashMap<usize, Lifetime> = HashMap::new();
	let mut lifetimes: Vec<Lifetime> = Vec::new();

	// Find all the lifetimes.
	for i in 0..code.len() {

		let mut replacement = None;

		if let Some(captures) = RE.captures(&code[i]) {
			let dash_count = captures[2].len();
			let is_start = &captures[3] == r"\";
			let comment = &captures[4];
			
			if unfinished_lifetimes.contains_key(&dash_count) {
				if is_start {
					panic!("Lifetime from line {} not finished before it is started again on line {}", unfinished_lifetimes[&dash_count].starting_line, i);
				} else {
					let mut lt = unfinished_lifetimes.remove(&dash_count).unwrap();
					lt.ending_line = i;
					lifetimes.push(lt);
				}
			} else {
				if is_start {
					unfinished_lifetimes.insert(dash_count, Lifetime {
						starting_line: i,
						ending_line: 0,
						comment: comment.to_string(),
					});
				} else {
					panic!("Ending lifetime on line {} wasn't started", i);
				}
			}

			replacement = Some(captures[1].to_string());
		}

		if let Some(replacement) = replacement {
			code[i] = replacement;
		}
	}

	lifetimes
}

// Generate an SVG. `code` should have lifetime comments removed.
fn generate_svg(code: &Vec<String>, lifetimes: &Vec<Lifetime>) -> String {
	use svg::Document;
	use svg::node::element::Text;
	use svg::node::element::Definitions;
	use svg::node::element::Style;
	use svg::node::element::Path;
	use svg::node::element::path::Data;
	use svg::Node;


	let mut document = Document::new(); //.set("viewBox", (0, 0, 200, 200));

	// Set up styles. This could be done in a separate CSS file too.

	let css = r#"<![CDATA[

.code {
	font-family: monospace;
	font-size: 16;
	white-space: pre;
	tab-size: 4;
}

.annotation {
	font-size: 16;
}

.m_code {
	font-family: monospace;
}

.m_italic {
	font-style: italic;
}

.m_underline {
	text-decoration: underline;
}

.m_bold {
	font-weight: bold;
}

.line {
	fill: none;
	stroke: black;
	stroke-width: 2;
	stroke-linecap: round;
	stroke-linejoin: round;
}

]]>"#;

	let defs = Definitions::new().add(Style::new(css));

	document.append(defs);

	// Draw the code.
	for i in 0..code.len() {
		// Empty lines are included to make text selection nicer.

		// I should probably use tspan elements.

		let text = Text::new()
						.set("x", 0)
						.set("y", i*20 + 20)
						.set("class", "code")
						.add(svg::node::Text::new(format!("{}", Escape(&code[i]))));

		document.append(text);
	}

	// Now draw all the lifetimes.



	for (i, lifetime) in lifetimes.iter().enumerate() {

		let y0 = lifetime.starting_line*20 + 10;
		let ym = (lifetime.starting_line+1)*20;
		let y1 = lifetime.ending_line*20 + 30;

		let x0 = 150;
		let xm = 230 + 20 * i;
		let x1 = 150;

		// Text
		let text = Text::new()
						.set("x", xm + 10)
						.set("y", y0 + 15)
						.set("class", "annotation")
						.add(svg::node::Text::new(markup_to_svg(&lifetime.comment)));

		document.append(text);



		// Lines. 
		let data = Data::new()
						.move_to((x0, y0))
						.line_to(((x0+xm)/2, y0))
						.line_to(((x0+xm)/2, ym))
						.line_to((xm, ym))
						.line_to(((x0+xm)/2, ym))
						.line_to(((x0+xm)/2, y1))
						.line_to((x1, y1));

		let path = Path::new()
						.set("class", "line")
						.set("d", data);

		document.append(path);
	}

	
	
	format!("{}", document)
}

// Convert simple markdown-ish markup to an SVG Text element contents, using <tspan> elements.
// Only _ (underline), * (bold), / (italic) and ` (code) are supported. Also standard backslash escaping. For example:
//
//  *Hello* `world!`
//
// is converted to
//
//  <tspan class="m_bold">Hello</tspan> <tspan class="m_code">world!</tspan>
//
// And
//
//     *foo _bar* baz_
//
// Is converted to
//
// <tspan class="m_bold">foo </tspan><tspan class="m_bold m_underline">bar</tspan><tspan class="m_underline"> baz</tspan
fn markup_to_svg(markup: &str) -> String {
	// The approach is simple - keep track of whether any of the formattings are activated.
	// Then, whenever one is toggled we check if any were previously active.
	// If they were, close the previous tspan. 
	// Then, open a new tspan with the new formatting.

	let mut in_bold = false;
	let mut in_underline = false;
	let mut in_italic = false;
	let mut in_code = false;

	let mut output = String::new();



	let mut escaped = false;

	for c in markup.chars() {
		if escaped {
			output.push(c);
			escaped = false;
			continue;
		}
		match c {
			'_' | '`' | '*' | '/' => {
				if in_bold || in_underline || in_italic || in_code {
					output += "</tspan>";
				}
				
				match c {
					'_' => in_underline = !in_underline,
					'`' => in_code = !in_code,
					'*' => in_bold = !in_bold,
					'/' => in_italic = !in_italic,
					_ => unreachable!(),
				}

				if in_bold || in_underline || in_italic || in_code {
					output += &format!("<tspan class=\"{} {} {} {}\">",
						if in_bold { "m_bold" } else { "" },
						if in_underline { "m_underline" } else { "" },
						if in_italic { "m_italic" } else { "" },
						if in_code { "m_code" } else { "" });
				}
			}
			'\\' => escaped = true,
			x => output.push(x),
		}
	}

	output
}

// Approach:
//
//  1. Read all data in from stdin.
//  2. Split into lines.
//  3. Find `// -` in each line using regex.
//  ???
//  4. Generate SVG.

struct Lifetime {
	starting_line: usize,
	ending_line: usize,
	comment: String,
}

fn run() -> Result<String, String> {
	// Read all data in from stdin.
	let mut buffer = String::new();
	io::stdin().read_to_string(&mut buffer).map_err(|x| x.to_string())?;

	// Split into lines.
	let mut code = buffer.lines().map(|x| x.into()).collect();

	let lifetimes = find_lifetimes(&mut code);

	let svg = generate_svg(&code, &lifetimes);

	Ok(svg)
}

fn main() {
	match run() {
		Ok(s) => println!("{}", s),
		Err(s) => println!("Error: {}", s),
	}
}
