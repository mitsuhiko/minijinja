// Date and time filters implemented with the native `Date` and `Intl` APIs.
//
// These mirror the `datetime` feature of minijinja-contrib (`datetimeformat`,
// `dateformat`, `timeformat` and `now`) but use the time zone database of the
// JavaScript runtime instead of bundling one.
import { TemplateError, passState } from "./shared.js";

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];
const WEEKDAYS = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
];

const DATETIME_FORMATS = {
  short: "%Y-%m-%d %H:%M",
  medium: "%b %-d %Y %H:%M",
  long: "%B %-d %Y %H:%M:%S",
  full: "%A, %B %-d %Y %H:%M:%S.%f",
  iso: "%Y-%m-%dT%H:%M:%S%:z",
  unix: "%s",
};
const TIME_FORMATS = {
  short: "%H:%M",
  medium: "%H:%M",
  long: "%H:%M:%S",
  full: "%H:%M:%S.%f",
  iso: "%Y-%m-%dT%H:%M:%S%:z",
  unix: "%s",
};
const DATE_FORMATS = {
  short: "%Y-%m-%d",
  medium: "%b %-d %Y",
  long: "%B %-d %Y",
  full: "%A, %B %-d %Y",
};

const ISO_RE =
  /^([+-]?\d{4,6})-(\d{2})-(\d{2})(?:[Tt ](\d{2}):(\d{2})(?::(\d{2})(?:[.,](\d{1,9}))?)?(Z|z|[+-]\d{2}(?::?\d{2})?)?)?(?:\[!?([^\]]+)\])?$/;

function fail(message, kind = "InvalidOperation") {
  throw new TemplateError(message, { kind });
}

// -- time zones --

const formatters = new Map();

function zoneFormatter(timeZone) {
  let formatter = formatters.get(timeZone);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat("en-US", {
      timeZone,
      hourCycle: "h23",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
      second: "numeric",
      era: "short",
      timeZoneName: "short",
    });
    formatters.set(timeZone, formatter);
  }
  return formatter;
}

function getZone(name) {
  if (name === "UTC" || name === "Etc/UTC") {
    return { offset: 0, name: "UTC" };
  }
  try {
    zoneFormatter(name);
  } catch {
    fail(`unknown timezone '${name}'`);
  }
  return { name };
}

/** Returns the UTC milliseconds for civil fields (supports all years). */
function utcMillis(year, month, day, hour = 0, minute = 0, second = 0) {
  const date = new Date(Date.UTC(2000, month - 1, day, hour, minute, second));
  date.setUTCFullYear(year);
  return date.getTime();
}

/** Returns the civil fields of an instant in a zone. */
function civilFields(ms, zone) {
  if (zone.offset !== undefined) {
    const date = new Date(ms + zone.offset * 60000);
    return {
      year: date.getUTCFullYear(),
      month: date.getUTCMonth() + 1,
      day: date.getUTCDate(),
      hour: date.getUTCHours(),
      minute: date.getUTCMinutes(),
      second: date.getUTCSeconds(),
      offset: zone.offset,
      abbreviation: zone.name ?? formatOffset(zone.offset, true),
    };
  }
  const parts = {};
  for (const part of zoneFormatter(zone.name).formatToParts(new Date(ms))) {
    parts[part.type] = part.value;
  }
  let year = Number(parts.year);
  if (parts.era === "BC" || parts.era === "B") {
    year = 1 - year;
  }
  const fields = {
    year,
    month: Number(parts.month),
    day: Number(parts.day),
    hour: Number(parts.hour),
    minute: Number(parts.minute),
    second: Number(parts.second),
  };
  const wall = utcMillis(
    fields.year,
    fields.month,
    fields.day,
    fields.hour,
    fields.minute,
    fields.second,
  );
  fields.offset = Math.round((wall - Math.floor(ms / 1000) * 1000) / 60000);
  fields.abbreviation = parts.timeZoneName;
  return fields;
}

/** Converts civil fields in a zone into an instant (milliseconds). */
function toInstant(fields, zone) {
  const wall = utcMillis(
    fields.year,
    fields.month,
    fields.day,
    fields.hour,
    fields.minute,
    fields.second,
  );
  if (zone.offset !== undefined) {
    return wall - zone.offset * 60000;
  }
  const first = civilFields(wall, zone).offset;
  const instant = wall - first * 60000;
  const second = civilFields(instant, zone).offset;
  return second === first ? instant : wall - second * 60000;
}

// -- parsing --

function parseOffset(text) {
  if (text === "Z" || text === "z") {
    return 0;
  }
  const sign = text[0] === "-" ? -1 : 1;
  const digits = text.slice(1).replace(":", "");
  return (
    sign * (Number(digits.slice(0, 2)) * 60 + Number(digits.slice(2) || 0))
  );
}

function parseValue(value) {
  if (typeof value === "number" || typeof value === "bigint") {
    const seconds = Number(value);
    const whole = Math.floor(seconds);
    return {
      kind: "zoned",
      ms: whole * 1000,
      nanos: Math.round((seconds - whole) * 1e9),
      zone: { offset: 0, name: "UTC" },
    };
  }
  if (typeof value !== "string" && !(value instanceof String)) {
    fail("value is not a datetime");
  }
  const match = ISO_RE.exec(String(value));
  if (!match) {
    fail("not a valid date or timestamp");
  }
  const [
    ,
    year,
    month,
    day,
    hour,
    minute,
    second,
    fraction,
    offset,
    annotation,
  ] = match;
  const fields = {
    year: Number(year),
    month: Number(month),
    day: Number(day),
    hour: Number(hour ?? 0),
    minute: Number(minute ?? 0),
    second: Number(second ?? 0),
  };
  if (
    fields.month < 1 ||
    fields.month > 12 ||
    fields.day < 1 ||
    fields.day > 31 ||
    fields.hour > 23 ||
    fields.minute > 59 ||
    fields.second > 60
  ) {
    fail("not a valid date or timestamp");
  }
  if (hour === undefined) {
    return { kind: "date", fields };
  }
  const nanos = fraction ? Number(fraction.padEnd(9, "0")) : 0;
  const annotatedZone = annotation ? getZone(annotation) : null;
  if (offset !== undefined) {
    const offsetMinutes = parseOffset(offset);
    const ms = toInstant(fields, { offset: offsetMinutes });
    return {
      kind: "zoned",
      ms,
      nanos,
      zone: annotatedZone ?? { offset: offsetMinutes },
    };
  }
  if (annotatedZone) {
    return {
      kind: "zoned",
      ms: toInstant(fields, annotatedZone),
      nanos,
      zone: annotatedZone,
    };
  }
  return { kind: "civil", fields, nanos };
}

function requestedZone(state, kwargs) {
  const name = kwargs.tz ?? state.lookup("TIMEZONE") ?? "original";
  if (typeof name !== "string" && !(name instanceof String)) {
    fail("timezone must be a string");
  }
  return String(name) === "original" ? null : getZone(String(name));
}

function toDateTime(state, value, kwargs, allowDate) {
  const parsed = parseValue(value);
  if (parsed.kind === "date") {
    if (!allowDate) {
      fail("filter requires time, but only received a date");
    }
    const zone = { offset: 0, name: "UTC" };
    return { ms: toInstant(parsed.fields, zone), nanos: 0, zone };
  }
  const tz = requestedZone(state, kwargs);
  if (parsed.kind === "civil") {
    const zone = tz ?? { offset: 0, name: "UTC" };
    return { ms: toInstant(parsed.fields, zone), nanos: parsed.nanos, zone };
  }
  return { ms: parsed.ms, nanos: parsed.nanos, zone: tz ?? parsed.zone };
}

// -- formatting --

function formatOffset(offset, colon) {
  const sign = offset < 0 ? "-" : "+";
  const abs = Math.abs(offset);
  const hours = String(Math.floor(abs / 60)).padStart(2, "0");
  const minutes = String(abs % 60).padStart(2, "0");
  return `${sign}${hours}${colon ? ":" : ""}${minutes}`;
}

function pad(value, width, flag, defaultPad = "0") {
  const text = String(value);
  if (flag === "-") {
    return text;
  }
  const padChar = flag === "_" ? " " : flag === "0" ? "0" : defaultPad;
  return text.padStart(width, padChar);
}

function dayOfYear(fields) {
  return (
    Math.round(
      (utcMillis(fields.year, fields.month, fields.day) -
        utcMillis(fields.year, 1, 1)) /
        86400000,
    ) + 1
  );
}

/** Formats a date time with a strftime style format string. */
function strftime(datetime, format) {
  const fields = civilFields(datetime.ms, datetime.zone);
  const weekday = new Date(
    utcMillis(fields.year, fields.month, fields.day),
  ).getUTCDay();
  const hour12 = fields.hour % 12 === 0 ? 12 : fields.hour % 12;
  let out = "";

  for (let i = 0; i < format.length; i++) {
    const char = format[i];
    if (char !== "%") {
      out += char;
      continue;
    }
    let j = i + 1;
    let flag = "";
    if ("-_0^#".includes(format[j] ?? "")) {
      flag = format[j++];
    }
    let width = "";
    while (/\d/.test(format[j] ?? "")) {
      width += format[j++];
    }
    let colon = false;
    if (format[j] === ":") {
      colon = true;
      j++;
    }
    let dot = false;
    if (format[j] === ".") {
      dot = true;
      j++;
    }
    const spec = format[j];
    i = j;

    let text;
    switch (spec) {
      case "Y":
        text = pad(fields.year, 4, flag);
        break;
      case "C":
        text = pad(Math.floor(fields.year / 100), 2, flag);
        break;
      case "y":
        text = pad(((fields.year % 100) + 100) % 100, 2, flag);
        break;
      case "m":
        text = pad(fields.month, 2, flag);
        break;
      case "B":
        text = MONTHS[fields.month - 1];
        break;
      case "b":
      case "h":
        text = MONTHS[fields.month - 1].slice(0, 3);
        break;
      case "d":
        text = pad(fields.day, 2, flag);
        break;
      case "e":
        text = pad(fields.day, 2, flag, " ");
        break;
      case "j":
        text = pad(dayOfYear(fields), 3, flag);
        break;
      case "A":
        text = WEEKDAYS[weekday];
        break;
      case "a":
        text = WEEKDAYS[weekday].slice(0, 3);
        break;
      case "u":
        text = String(weekday === 0 ? 7 : weekday);
        break;
      case "w":
        text = String(weekday);
        break;
      case "H":
        text = pad(fields.hour, 2, flag);
        break;
      case "k":
        text = pad(fields.hour, 2, flag, " ");
        break;
      case "I":
        text = pad(hour12, 2, flag);
        break;
      case "l":
        text = pad(hour12, 2, flag, " ");
        break;
      case "M":
        text = pad(fields.minute, 2, flag);
        break;
      case "S":
        text = pad(fields.second, 2, flag);
        break;
      case "p":
        text = fields.hour < 12 ? "AM" : "PM";
        break;
      case "P":
        text = fields.hour < 12 ? "am" : "pm";
        break;
      case "f": {
        const digits = String(datetime.nanos).padStart(9, "0");
        if (width) {
          text = digits.slice(0, Number(width)).padEnd(Number(width), "0");
        } else {
          text = digits.replace(/0+$/, "") || "0";
        }
        if (dot) {
          text = datetime.nanos === 0 && !width ? "" : `.${text}`;
        }
        break;
      }
      case "s":
        text = String(Math.floor(datetime.ms / 1000));
        break;
      case "z":
        text = formatOffset(fields.offset, colon);
        break;
      case "Z":
        text = fields.abbreviation ?? formatOffset(fields.offset, true);
        break;
      case "F":
        text = strftime(datetime, "%Y-%m-%d");
        break;
      case "D":
        text = strftime(datetime, "%m/%d/%y");
        break;
      case "T":
        text = strftime(datetime, "%H:%M:%S");
        break;
      case "R":
        text = strftime(datetime, "%H:%M");
        break;
      case "r":
        text = strftime(datetime, "%I:%M:%S %p");
        break;
      case "c":
        text = strftime(datetime, "%a %b %e %H:%M:%S %Y");
        break;
      case "n":
        text = "\n";
        break;
      case "t":
        text = "\t";
        break;
      case "%":
        text = "%";
        break;
      default:
        fail("invalid format string");
    }
    if (flag === "^") {
      text = text.toUpperCase();
    } else if (flag === "#") {
      text =
        text === text.toUpperCase() ? text.toLowerCase() : text.toUpperCase();
    }
    out += text;
  }
  return out;
}

function parseArgs(args) {
  if (args.length > 1) {
    fail("too many arguments", "TooManyArguments");
  }
  const kwargs = args[0] ?? {};
  if (typeof kwargs !== "object" || kwargs === null || Array.isArray(kwargs)) {
    fail("too many arguments", "TooManyArguments");
  }
  for (const key of Object.keys(kwargs)) {
    if (key !== "format" && key !== "tz") {
      fail(`unknown keyword argument '${key}'`, "TooManyArguments");
    }
  }
  return kwargs;
}

function makeFilter(formats, configKey, allowDate) {
  return passState((state, value, ...args) => {
    const kwargs = parseArgs(args);
    const datetime = toDateTime(state, value, kwargs, allowDate);
    const format = String(kwargs.format ?? state.lookup(configKey) ?? "medium");
    return strftime(datetime, formats[format] ?? format);
  });
}

/**
 * Registers the date and time filters and functions with an environment.
 *
 * This adds the `datetimeformat`, `dateformat` and `timeformat` filters and
 * the `now` function.  They behave like the ones of the `datetime` feature
 * of minijinja-contrib but use the time zone support of the JavaScript
 * runtime.
 */
export function addDateTimeSupport(env) {
  env.addFilter(
    "datetimeformat",
    makeFilter(DATETIME_FORMATS, "DATETIME_FORMAT", false),
  );
  env.addFilter("timeformat", makeFilter(TIME_FORMATS, "TIME_FORMAT", false));
  env.addFilter("dateformat", makeFilter(DATE_FORMATS, "DATE_FORMAT", true));
  env.addFunction("now", () => Date.now() / 1000);
}
