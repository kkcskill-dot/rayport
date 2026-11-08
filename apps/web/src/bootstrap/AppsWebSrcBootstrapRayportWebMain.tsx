import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createHttpRayportApi } from '../infrastructure/http/AppsWebSrcInfrastructureHttpRayportApi';
import RayportApp from '../presentation/AppsWebSrcPresentationRayportApp';
import '../presentation/styles/AppsWebSrcPresentationStylesRayport.css';

const api = createHttpRayportApi();
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RayportApp api={api} />
  </StrictMode>,
);
