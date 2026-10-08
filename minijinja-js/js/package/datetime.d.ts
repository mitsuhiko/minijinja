import type { Environment } from "./types.js";

/**
 * Registers the date and time filters and functions with an environment.
 *
 * This adds the `datetimeformat`, `dateformat` and `timeformat` filters and
 * the `now` function.  They behave like the ones of the `datetime` feature
 * of minijinja-contrib but use the time zone support of the JavaScript
 * runtime (`Intl`).
 *
 * The filters accept Unix timestamps (seconds), ISO 8601 strings
 * (optionally with a time zone annotation such as `[Europe/Vienna]`) and
 * `Date` objects.  The `format` and `tz` keyword arguments default to the
 * `DATETIME_FORMAT`, `DATE_FORMAT`, `TIME_FORMAT` and `TIMEZONE` globals.
 */
export declare function addDateTimeSupport(env: Environment): void;
