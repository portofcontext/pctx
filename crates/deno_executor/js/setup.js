// Import all deno_web modules to ensure they are evaluated
import "ext:deno_webidl/00_webidl.js";
import "ext:deno_console/01_console.js";
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";
import "ext:deno_web/00_infra.js";
import "ext:deno_web/01_dom_exception.js";
import "ext:deno_web/01_mimesniff.js";
import "ext:deno_web/02_event.js";
import "ext:deno_web/02_structured_clone.js";
import "ext:deno_web/02_timers.js";
import "ext:deno_web/03_abort_signal.js";
import "ext:deno_web/05_base64.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import "ext:deno_web/09_file.js";
import "ext:deno_web/10_filereader.js";
import "ext:deno_web/12_location.js";
import "ext:deno_web/13_message_port.js";
import "ext:deno_web/14_compression.js";
import "ext:deno_web/15_performance.js";
import "ext:deno_web/16_image_data.js";
import "ext:deno_web/04_global_interfaces.js";

// Expose Web APIs to global scope
globalThis.URL = url.URL;
globalThis.URLSearchParams = url.URLSearchParams;
globalThis.URLPattern = urlPattern.URLPattern;

// Expose Streams API
globalThis.ReadableStream = streams.ReadableStream;
globalThis.WritableStream = streams.WritableStream;
globalThis.TransformStream = streams.TransformStream;
globalThis.ByteLengthQueuingStrategy = streams.ByteLengthQueuingStrategy;
globalThis.CountQueuingStrategy = streams.CountQueuingStrategy;
globalThis.ReadableStreamDefaultReader = streams.ReadableStreamDefaultReader;
globalThis.ReadableStreamBYOBReader = streams.ReadableStreamBYOBReader;
globalThis.ReadableStreamBYOBRequest = streams.ReadableStreamBYOBRequest;
globalThis.ReadableByteStreamController = streams.ReadableByteStreamController;
globalThis.ReadableStreamDefaultController = streams.ReadableStreamDefaultController;
globalThis.TransformStreamDefaultController = streams.TransformStreamDefaultController;
globalThis.WritableStreamDefaultWriter = streams.WritableStreamDefaultWriter;
globalThis.WritableStreamDefaultController = streams.WritableStreamDefaultController;

// Expose Encoding API
globalThis.TextEncoder = encoding.TextEncoder;
globalThis.TextDecoder = encoding.TextDecoder;
globalThis.TextDecoderStream = encoding.TextDecoderStream;
globalThis.TextEncoderStream = encoding.TextEncoderStream;

// Set up console output capturing
globalThis.__stdout = [];
globalThis.__stderr = [];

// Override console.log to capture stdout
console.log = (...args) => {
    const message = args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
    globalThis.__stdout.push(message);
};

// Override console.error to capture stderr
console.error = (...args) => {
    const message = args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
    globalThis.__stderr.push(message);
};

// console.warn, console.info, and console.debug also go to stderr
console.warn = (...args) => {
    const message = args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
    globalThis.__stderr.push(message);
};

console.info = (...args) => {
    const message = args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
    globalThis.__stdout.push(message);
};

console.debug = (...args) => {
    const message = args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
    globalThis.__stdout.push(message);
};
