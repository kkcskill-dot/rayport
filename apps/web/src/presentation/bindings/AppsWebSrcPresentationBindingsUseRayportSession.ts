import { useCallback, useEffect, useRef, useState } from 'react';
import {
  RayportApiError,
  type RayportApi,
} from '../../application/ports/AppsWebSrcApplicationPortsRayportApi';
import type { Principal, Workspace } from '../../domain/AppsWebSrcDomainRayportTypes';

export function useRayportSession(api: RayportApi) {
  const [principal, setPrincipal] = useState<Principal | null>(null);
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [initializing, setInitializing] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const inFlight = useRef(false);
  const report = useCallback((cause: unknown) => {
    if (cause instanceof RayportApiError && cause.status === 401) {
      setPrincipal(null);
      setWorkspace(null);
    }
    setError(cause instanceof Error ? cause.message : '처리 중 오류가 발생했습니다.');
  }, []);
  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const identity = await api.session();
        if (!active) return;
        setPrincipal(identity);
        const data = await api.workspace();
        if (active) setWorkspace(data);
      } catch (cause) {
        if (active && !(cause instanceof RayportApiError && cause.status === 401)) report(cause);
      } finally {
        if (active) setInitializing(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [api, report]);

  async function run<T>(
    action: () => Promise<T>,
    success = '',
    refresh = true,
  ): Promise<T | undefined> {
    if (inFlight.current) return undefined;
    inFlight.current = true;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const result = await action();
      if (refresh) {
        try {
          setWorkspace(await api.workspace());
        } catch (cause) {
          report(cause);
          setNotice(
            '작업은 처리되었지만 목록 갱신에 실패했습니다. 새로고침으로 상태를 확인하세요.',
          );
          return result;
        }
      }
      setNotice(success);
      return result;
    } catch (cause) {
      report(cause);
      return undefined;
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  async function login(token: string): Promise<boolean> {
    const identity = await run(() => api.login(token), '', false);
    if (!identity) return false;
    setPrincipal(identity);
    await run(() => api.workspace(), '', false).then((data) => {
      if (data) setWorkspace(data);
    });
    return true;
  }
  async function logout() {
    await run(
      async () => {
        await api.logout();
        setPrincipal(null);
        setWorkspace(null);
      },
      '',
      false,
    );
  }
  async function refresh() {
    const data = await run(() => api.workspace(), '최신 상태를 불러왔습니다.', false);
    if (data) setWorkspace(data);
  }
  return {
    principal,
    workspace,
    initializing,
    busy,
    error,
    notice,
    run,
    login,
    logout,
    refresh,
    dismiss: () => {
      setError('');
      setNotice('');
    },
  };
}
