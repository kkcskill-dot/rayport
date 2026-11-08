import { describe, expect, it, vi } from 'vitest';
import { createHttpRayportApi } from '../src/infrastructure/http/AppsWebSrcInfrastructureHttpRayportApi';

function response(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}
describe('HTTP boundary', () => {
  it('uses same-origin HttpOnly session semantics and explicit CSRF header', async () => {
    const fetcher = vi.fn(async () => response({ data: { id: 'a' } }));
    const api = createHttpRayportApi(fetcher);
    await api.login('private-token');
    expect(fetcher).toHaveBeenCalledWith(
      '/api/v1/session',
      expect.objectContaining({
        method: 'POST',
        credentials: 'same-origin',
        headers: expect.objectContaining({ 'X-Rayport-Request': '1' }),
        body: '{"token":"private-token"}',
      }),
    );
    await api.logout();
    expect(fetcher).toHaveBeenLastCalledWith(
      '/api/v1/session',
      expect.objectContaining({
        method: 'DELETE',
        headers: expect.objectContaining({ 'X-Rayport-Request': '1' }),
      }),
    );
  });
  it.each([401, 403, 409, 422, 503])('retains typed server errors (%i)', async (status) => {
    const api = createHttpRayportApi(
      vi.fn(async () =>
        response({ error: { code: 'blocked', message: '정책으로 차단했습니다.' } }, status),
      ),
    );
    await expect(api.workspace()).rejects.toMatchObject({
      status,
      code: 'blocked',
      message: '정책으로 차단했습니다.',
    });
  });
  it('handles non-JSON errors and connection loss without leaking request data', async () => {
    const broken = createHttpRayportApi(
      vi.fn(async () => new Response('<html>proxy</html>', { status: 502 })),
    );
    await expect(broken.workspace()).rejects.toMatchObject({ code: 'invalid_response' });
    const disconnected = createHttpRayportApi(
      vi.fn(async () => {
        throw new Error('contains secrets');
      }),
    );
    await expect(disconnected.workspace()).rejects.toMatchObject({
      code: 'network_error',
      message: expect.not.stringContaining('secrets'),
    });
  });
  it('rejects missing envelope and unsafe integer amounts', async () => {
    await expect(
      createHttpRayportApi(vi.fn(async () => response({ wrong: 1 }))).workspace(),
    ).rejects.toMatchObject({ code: 'invalid_response' });
    const api = createHttpRayportApi(
      vi.fn(async () =>
        response({
          data: {
            rows: [],
            gross_minor: Number.MAX_SAFE_INTEGER + 1,
            cancel_minor: 0,
            net_minor: 1,
          },
        }),
      ),
    );
    await expect(
      api.query('tool', {
        start_date: '2026-01-01',
        end_date_exclusive: '2026-01-02',
        store_ids: [],
      }),
    ).rejects.toMatchObject({ code: 'unsafe_amount' });
  });
  it('encodes IDs as a single path segment', async () => {
    const fetcher = vi.fn<typeof fetch>(async () => response({ data: {} }));
    const api = createHttpRayportApi(fetcher);
    await api.publishTool('one/two?admin=true');
    expect(fetcher.mock.calls[0][0]).toBe('/api/v1/tools/one%2Ftwo%3Fadmin%3Dtrue/publish');
  });
});
