import { useEffect, useState, type FormEvent } from 'react';
import type { RayportApi } from '../application/ports/AppsWebSrcApplicationPortsRayportApi';
import type { RayportPage } from '../domain/AppsWebSrcDomainRayportTypes';
import { useRayportSession } from './bindings/AppsWebSrcPresentationBindingsUseRayportSession';
import RayportAskPage from './pages/AppsWebSrcPresentationPagesRayportAskPage';
import RayportCatalogPage from './pages/AppsWebSrcPresentationPagesRayportCatalogPage';
import RayportCandidatesPage from './pages/AppsWebSrcPresentationPagesRayportCandidatesPage';
import RayportMartsPage from './pages/AppsWebSrcPresentationPagesRayportMartsPage';
import RayportRequestsPage from './pages/AppsWebSrcPresentationPagesRayportRequestsPage';
import RayportOperationsPage from './pages/AppsWebSrcPresentationPagesRayportOperationsPage';

const navigation: { id: RayportPage; label: string; icon: string; admin?: boolean }[] = [
  { id: 'ask', label: '업무 질문', icon: '◈' },
  { id: 'catalog', label: '업무 API', icon: '▦' },
  { id: 'requests', label: '생성 요청', icon: '↗' },
  { id: 'candidates', label: 'API 발굴', icon: '⌘', admin: true },
  { id: 'marts', label: '데이터 마트', icon: '▥', admin: true },
  { id: 'operations', label: '운영과 정책', icon: '◎', admin: true },
];
const roleLabels = { admin: '관리자', analyst: '분석가', viewer: '조회 사용자' };
export default function RayportApp({ api }: { api: RayportApi }) {
  const session = useRayportSession(api);
  const [page, setPage] = useState<RayportPage>('ask');
  const [token, setToken] = useState('');
  const [question, setQuestion] = useState('');
  useEffect(() => {
    if (!session.principal) {
      setQuestion('');
      setPage('ask');
    }
  }, [session.principal]);
  async function login(event: FormEvent) {
    event.preventDefault();
    if (await session.login(token)) setToken('');
  }
  const notifications = (
    <>
      {session.error && (
        <div className="alert error" role="alert">
          {session.error}
          <button type="button" onClick={session.dismiss} aria-label="오류 닫기">
            ×
          </button>
        </div>
      )}
      {session.notice && (
        <div className="alert success" role="status">
          {session.notice}
        </div>
      )}
    </>
  );
  if (session.initializing)
    return (
      <div className="login-shell" role="status">
        로그인 상태를 확인하고 있습니다…
      </div>
    );
  if (!session.principal)
    return (
      <main className="login-shell">
        <section className="panel login-panel">
          <div className="brand">
            <span className="brand-mark">r</span>rayport
          </div>
          <h1>업무 공간 로그인</h1>
          <p className="subtitle">운영자가 발급한 접근 토큰으로 로그인하세요.</p>
          {notifications}
          <form onSubmit={(event) => void login(event)}>
            <label className="field">
              <span>접근 토큰</span>
              <input
                autoComplete="off"
                type="password"
                value={token}
                onChange={(event) => setToken(event.target.value)}
                required
                maxLength={512}
                autoFocus
              />
            </label>
            <button className="button primary" disabled={session.busy || !token.trim()}>
              {session.busy ? '확인 중…' : '로그인'}
            </button>
          </form>
          <p className="muted section-gap">접근 토큰은 브라우저 저장소에 보관하지 않습니다.</p>
        </section>
      </main>
    );
  const principal = session.principal;
  const admin = principal.role === 'admin';
  const activePage = navigation.find((item) => item.id === page && (!item.admin || admin))
    ? page
    : 'ask';
  const props = session.workspace
    ? { api, workspace: session.workspace, principal, run: session.run }
    : null;
  const pendingCount =
    session.workspace?.requests.filter(
      (request) => !['resolved', 'rejected'].includes(request.status),
    ).length ?? 0;
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">r</span>
          <span>
            rayport<small>업무 데이터</small>
          </span>
        </div>
        <div className="workspace">
          <div>
            {principal.tenant_id}
            <small>{roleLabels[principal.role]}</small>
          </div>
        </div>
        <nav aria-label="주요 메뉴">
          {navigation
            .filter((item) => !item.admin || admin)
            .map((item) => (
              <div key={item.id}>
                <button
                  className={`nav-item ${activePage === item.id ? 'active' : ''}`}
                  aria-current={activePage === item.id ? 'page' : undefined}
                  onClick={() => {
                    setPage(item.id);
                    session.dismiss();
                  }}
                >
                  <span>{item.icon}</span>
                  {item.label}
                  {item.id === 'requests' && pendingCount > 0 && (
                    <span className="nav-count">{pendingCount}</span>
                  )}
                </button>
              </div>
            ))}
        </nav>
        <div className="sidebar-note">
          <span className="status-dot" />
          서버 연결
          <p>
            데이터는 권한과 기준 시각을
            <br />
            확인한 뒤 제공합니다.
          </p>
        </div>
        <div className="profile">
          <span className="avatar">{principal.id.slice(0, 1)}</span>
          <div>
            {principal.id}
            <small>{roleLabels[principal.role]}</small>
          </div>
        </div>
      </aside>
      <div className="workspace-main">
        <header className="topbar">
          <div>
            {principal.tenant_id}
            <span>/</span>
            <strong>{navigation.find((item) => item.id === activePage)?.label}</strong>
          </div>
          <div className="header-right">
            <button
              className="text-button"
              disabled={session.busy}
              onClick={() => void session.refresh()}
            >
              새로고침
            </button>
            <button
              className="text-button"
              disabled={session.busy}
              onClick={() => void session.logout()}
            >
              로그아웃
            </button>
          </div>
        </header>
        <main>
          {notifications}
          {session.busy && (
            <p className="busy-state" role="status">
              처리 중입니다…
            </p>
          )}
          {!props ? (
            <section className="panel">
              <h1>업무 공간을 불러오지 못했습니다.</h1>
              <button
                className="button primary"
                disabled={session.busy}
                onClick={() => void session.refresh()}
              >
                다시 불러오기
              </button>
            </section>
          ) : (
            <fieldset className="workspace-controls" disabled={session.busy} aria-label="업무 작업">
              {activePage === 'ask' && (
                <RayportAskPage
                  {...props}
                  initialQuestion={question}
                  onRequestCreated={() => setPage('requests')}
                />
              )}
              {activePage === 'catalog' && <RayportCatalogPage {...props} />}
              {activePage === 'candidates' && <RayportCandidatesPage {...props} />}
              {activePage === 'marts' && <RayportMartsPage {...props} />}
              {activePage === 'requests' && (
                <RayportRequestsPage
                  {...props}
                  onReopen={(value) => {
                    setQuestion(value);
                    setPage('ask');
                  }}
                />
              )}
              {activePage === 'operations' && <RayportOperationsPage {...props} />}
            </fieldset>
          )}
          <footer>레이포트 1.0 · 기준과 권한을 함께 확인하는 업무 데이터</footer>
        </main>
      </div>
    </div>
  );
}
