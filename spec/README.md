# API description and shared fixtures

Zindi does not publish an official API reference. This folder is the single source
of truth for the endpoints that the clients in this repository use.

* [`openapi.yaml`](openapi.yaml) describes the endpoints, parameters and the
  conventions of the platform (response envelope, error format, auth header).
* [`fixtures/`](fixtures) holds example responses. They follow the real shapes
  returned by `https://api.zindi.world/v1`, with fictional people and competitions.

**Every client's test suite loads these same fixtures.** When the platform changes,
update the description and the fixtures here first, then make each client's tests
pass again. That keeps the Python, JavaScript, R and Rust clients in step.

To check that the fixtures still match reality, run the opt-in live tests of the
Python client (see [`python/README.md`](../python/README.md#running-the-tests)).
