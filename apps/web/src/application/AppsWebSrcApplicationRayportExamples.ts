import type { DiscoveryImport, MartImport } from '../domain/AppsWebSrcDomainRayportTypes';

export function discoveryExample(): DiscoveryImport {
  return {
    source_label: '합성 SQL 관측 예제 · 실제 AWR 아님',
    diagnostics_authorized: true,
    observations: [
      {
        sql_text:
          'SELECT business_date, store_id, gross_minor, cancel_minor, currency FROM store_sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id',
        executions: 120,
        ash_samples: 8,
        module: 'synthetic-sales',
      },
    ],
  };
}
export function martExample(now = new Date()): MartImport {
  const date = new Date(now.getTime() - 86_400_000).toISOString().slice(0, 10);
  return {
    id: 'sample-sales',
    name: '합성 점포 매출 예제',
    data_as_of: now.toISOString(),
    complete_through: date,
    rows: [
      {
        business_date: date,
        store_id: 'S01',
        gross_minor: 120000,
        cancel_minor: 20000,
        currency: 'KRW',
      },
      {
        business_date: date,
        store_id: 'S02',
        gross_minor: 80000,
        cancel_minor: 5000,
        currency: 'KRW',
      },
    ],
  };
}
