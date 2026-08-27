/**
 * The HTTP statuses this client reasons about by name.
 *
 * Only the ones the app branches on live here; everything else is passed
 * through as an opaque number.
 */
export const HTTP_OK = 200;
export const HTTP_CREATED = 201;
export const HTTP_NO_CONTENT = 204;
export const HTTP_UNAUTHORIZED = 401;
export const HTTP_NOT_FOUND = 404;
export const HTTP_CONFLICT = 409;
export const HTTP_TOO_MANY_REQUESTS = 429;
export const HTTP_INTERNAL_ERROR = 500;
