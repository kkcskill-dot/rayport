import type {
  Policy,
  Principal,
  QueryInput,
  RequestStatus,
  Tool,
} from '../domain/AppsWebSrcDomainRayportTypes';

export function canManage(principal: Principal): boolean {
  return principal.role === 'admin';
}
export function splitIdentifiers(value: string): string[] {
  return [
    ...new Set(
      value
        .split(',')
        .map((part) => part.trim())
        .filter(Boolean),
    ),
  ];
}
export function validateQuery(input: QueryInput, tool: Tool): string | null {
  const iso = /^\d{4}-\d{2}-\d{2}$/;
  const validDate = (value: string) =>
    iso.test(value) &&
    !Number.isNaN(Date.parse(`${value}T00:00:00Z`)) &&
    new Date(`${value}T00:00:00Z`).toISOString().slice(0, 10) === value;
  if (!validDate(input.start_date) || !validDate(input.end_date_exclusive))
    return '올바른 시작일과 종료일을 입력하세요.';
  const days = (Date.parse(input.end_date_exclusive) - Date.parse(input.start_date)) / 86_400_000;
  if (days <= 0) return '종료일은 시작일보다 뒤여야 합니다. 종료일 당일은 포함하지 않습니다.';
  if (days > tool.max_days) return `이 도구는 한 번에 ${tool.max_days}일까지 조회할 수 있습니다.`;
  return null;
}
export function validatePolicy(policy: Policy): string | null {
  if (
    ![policy.per_user_per_minute, policy.source_per_minute, policy.max_concurrent].every(
      (value) => Number.isSafeInteger(value) && value >= 1 && value <= 10_000,
    )
  )
    return '정책 한도는 1부터 10,000 사이의 정수여야 합니다.';
  return null;
}
export function parseImportJson(text: string): unknown {
  if (new TextEncoder().encode(text).length > 2_000_000)
    throw new Error('반입 파일은 2MB 이하여야 합니다.');
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('JSON 형식이 올바르지 않습니다.');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new Error('최상위 JSON은 객체여야 합니다.');
  return value;
}
export function formatWon(amount: number): string {
  if (!Number.isSafeInteger(amount)) return '표시 범위 초과';
  return `${new Intl.NumberFormat('ko-KR').format(amount)}원`;
}
export function formatTimestamp(value: string | number): string {
  const date = new Date(typeof value === 'number' ? value * 1000 : value);
  if (Number.isNaN(date.getTime())) return '시각 확인 필요';
  return `${new Intl.DateTimeFormat('ko-KR', { timeZone: 'Asia/Seoul', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).format(date)} KST`;
}
export const requestStatusLabels: Record<RequestStatus, string> = {
  submitted: '접수',
  in_review: '검토 중',
  building: '구현 중',
  ready: '사용 준비됨',
  resolved: '해결 확인',
  rejected: '반려',
};
export const toolStatusLabels = {
  draft: '초안',
  validated: '검증 완료',
  published: '게시',
  retired: '사용 중지',
} as const;
export function nextRequestStates(
  status: RequestStatus,
  admin: boolean,
  isRequester: boolean,
): RequestStatus[] {
  if (status === 'ready')
    return [...(admin ? ['building' as const] : []), ...(isRequester ? ['resolved' as const] : [])];
  if (!admin) return [];
  const next: Record<RequestStatus, RequestStatus[]> = {
    submitted: ['in_review', 'rejected'],
    in_review: ['building', 'rejected'],
    building: ['ready', 'rejected'],
    ready: [],
    resolved: [],
    rejected: [],
  };
  return next[status];
}
