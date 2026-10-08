import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { Environment } from "minijinja-js";
import { addDateTimeSupport } from "minijinja-js/datetime";

function render(source, ctx) {
  const env = new Environment();
  addDateTimeSupport(env);
  return env.renderStr(source, ctx);
}

// 2023-06-24T16:37:22Z
const TS = 1687624642;

describe("datetime", () => {
  it("should format timestamps with the builtin formats", () => {
    assert.equal(
      render("{{ ts|datetimeformat }}", { ts: TS }),
      "Jun 24 2023 16:37",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='short') }}", { ts: TS }),
      "2023-06-24 16:37",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='long') }}", { ts: TS }),
      "June 24 2023 16:37:22",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='full') }}", { ts: TS }),
      "Saturday, June 24 2023 16:37:22.0",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='iso') }}", { ts: TS }),
      "2023-06-24T16:37:22+00:00",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='unix') }}", { ts: TS }),
      String(TS),
    );
    assert.equal(render("{{ ts|timeformat }}", { ts: TS }), "16:37");
    assert.equal(render("{{ ts|dateformat }}", { ts: TS }), "Jun 24 2023");
  });

  it("should support strftime formats", () => {
    assert.equal(
      render(
        "{{ ts|datetimeformat(format='%a %e %b %y %I:%M:%S %p %j %%') }}",
        { ts: TS },
      ),
      "Sat 24 Jun 23 04:37:22 PM 175 %",
    );
    assert.throws(
      () => render("{{ ts|datetimeformat(format='%Q') }}", { ts: TS }),
      /invalid format string/,
    );
  });

  it("should convert time zones", () => {
    assert.equal(
      render("{{ ts|datetimeformat(format='iso', tz='Europe/Vienna') }}", {
        ts: TS,
      }),
      "2023-06-24T18:37:22+02:00",
    );
    assert.equal(
      render("{{ ts|datetimeformat(format='iso') }}", {
        ts: TS,
        TIMEZONE: "Asia/Tokyo",
      }),
      "2023-06-25T01:37:22+09:00",
    );
    assert.throws(
      () => render("{{ ts|datetimeformat(tz='Mars/Base') }}", { ts: TS }),
      /unknown timezone 'Mars\/Base'/,
    );
  });

  it("should parse ISO 8601 strings and dates", () => {
    assert.equal(
      render("{{ '2023-06-24T16:37:22+02:00'|datetimeformat(format='iso') }}"),
      "2023-06-24T16:37:22+02:00",
    );
    assert.equal(
      render(
        "{{ '2023-06-24T16:37:22'|datetimeformat(format='iso', tz='America/New_York') }}",
      ),
      "2023-06-24T16:37:22-04:00",
    );
    assert.equal(
      render("{{ '2023-01-10T12:00:00Z[Europe/Vienna]'|timeformat }}"),
      "13:00",
    );
    assert.equal(
      render("{{ d|datetimeformat(format='iso') }}", {
        d: new Date(TS * 1000),
      }),
      "2023-06-24T16:37:22+00:00",
    );
    assert.equal(
      render("{{ '2023-06-24'|dateformat(format='full') }}"),
      "Saturday, June 24 2023",
    );
    assert.throws(
      () => render("{{ '2023-06-24'|datetimeformat }}"),
      /requires time/,
    );
    assert.throws(() => render("{{ 'nope'|dateformat }}"), /not a valid date/);
  });

  it("should use configured formats", () => {
    assert.equal(
      render(
        "{{ ts|datetimeformat }} {{ ts|dateformat }} {{ ts|timeformat }}",
        {
          ts: TS,
          DATETIME_FORMAT: "short",
          DATE_FORMAT: "%d.%m.%Y",
          TIME_FORMAT: "long",
        },
      ),
      "2023-06-24 16:37 24.06.2023 16:37:22",
    );
  });

  it("should reject unknown arguments", () => {
    assert.throws(
      () => render("{{ ts|dateformat(foo=1) }}", { ts: TS }),
      /unknown keyword argument 'foo'/,
    );
  });

  it("should provide now", () => {
    const now = Number(render("{{ now() }}"));
    assert.ok(Math.abs(now - Date.now() / 1000) < 5);
  });
});
