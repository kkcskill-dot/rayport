import { useId, useState, type ReactNode } from 'react';
import { parseImportJson } from '../../application/AppsWebSrcApplicationRayportSession';
import type { RayportApi } from '../../application/ports/AppsWebSrcApplicationPortsRayportApi';
import type { Principal, Workspace } from '../../domain/AppsWebSrcDomainRayportTypes';

export type ActionRunner = <T>(
  action: () => Promise<T>,
  success?: string,
  refresh?: boolean,
) => Promise<T | undefined>;
export type WorkspacePageProps = {
  api: RayportApi;
  workspace: Workspace;
  principal: Principal;
  run: ActionRunner;
};
export function EmptyState({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="empty-state">
      <h3>{title}</h3>
      <p>{children}</p>
    </div>
  );
}
export function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  );
}
export function ErrorText({ children }: { children: string }) {
  return children ? (
    <p className="inline-error" role="alert">
      {children}
    </p>
  ) : null;
}
export function downloadJson(value: object, filename: string) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }),
  );
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
export function JsonImport({
  label,
  example,
  description,
  buttonLabel,
  onImport,
}: {
  label: string;
  example: object;
  description: string;
  buttonLabel: string;
  onImport: (value: unknown) => Promise<boolean>;
}) {
  const [text, setText] = useState('');
  const [error, setError] = useState('');
  const inputId = useId();
  async function submit() {
    setError('');
    try {
      const value = parseImportJson(text);
      if (await onImport(value)) setText('');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : '반입 형식을 확인하세요.');
    }
  }
  function downloadExample() {
    downloadJson(example, 'rayport-synthetic-example.json');
  }
  return (
    <section className="panel section-gap">
      <h2>{label}</h2>
      <p className="subtitle">{description}</p>
      <div className="action-row">
        <label className="button secondary" htmlFor={inputId}>
          JSON 파일 선택
        </label>
        <input
          id={inputId}
          type="file"
          accept="application/json,.json"
          className="file-input"
          onChange={async (event) => {
            const file = event.target.files?.[0];
            if (!file) return;
            if (file.size > 2_000_000) {
              setError('반입 파일은 2MB 이하여야 합니다.');
              return;
            }
            try {
              setText(await file.text());
              setError('');
            } catch {
              setError('파일을 읽지 못했습니다.');
            }
            event.target.value = '';
          }}
        />
        <button type="button" className="text-button" onClick={downloadExample}>
          합성 예제 내려받기
        </button>
      </div>
      <Field label="반입할 JSON">
        <textarea
          rows={9}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
            setError('');
          }}
          spellCheck={false}
          placeholder="반입 내용을 붙여 넣거나 파일을 선택하세요."
        />
      </Field>
      <ErrorText>{error}</ErrorText>
      <button
        type="button"
        className="button primary"
        disabled={!text.trim()}
        onClick={() => void submit()}
      >
        {buttonLabel}
      </button>
      <p className="muted section-gap">
        예제는 합성 데이터입니다. 내려받기만으로 서버에 등록되지 않습니다.
      </p>
    </section>
  );
}
