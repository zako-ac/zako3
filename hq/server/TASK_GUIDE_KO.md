# 태스크 가이드: User 기능의 `name`을 `username`으로 변경하기
AI가 씀. Reviewed by MincoMK.

## Mokcha
<!-- toc -->

- [Rust 기초 입문](#rust-%EA%B8%B0%EC%B4%88-%EC%9E%85%EB%AC%B8)
  * [알아야 할 Rust 기본 개념](#%EC%95%8C%EC%95%84%EC%95%BC-%ED%95%A0-rust-%EA%B8%B0%EB%B3%B8-%EA%B0%9C%EB%85%90)
  * [이 코드베이스의 일반적인 패턴](#%EC%9D%B4-%EC%BD%94%EB%93%9C%EB%B2%A0%EC%9D%B4%EC%8A%A4%EC%9D%98-%EC%9D%BC%EB%B0%98%EC%A0%81%EC%9D%B8-%ED%8C%A8%ED%84%B4)
- [HQ 코드베이스 아키텍처](#hq-%EC%BD%94%EB%93%9C%EB%B2%A0%EC%9D%B4%EC%8A%A4-%EC%95%84%ED%82%A4%ED%85%8D%EC%B2%98)
  * [주요 아키텍처 개념](#%EC%A3%BC%EC%9A%94-%EC%95%84%ED%82%A4%ED%85%8D%EC%B2%98-%EA%B0%9C%EB%85%90)
  * [데이터 흐름](#%EB%8D%B0%EC%9D%B4%ED%84%B0-%ED%9D%90%EB%A6%84)
- [일: `name` → `username` 변경하기 (Reviewer: 해 줘!)](#%EC%9D%BC-name-%E2%86%92-username-%EB%B3%80%EA%B2%BD%ED%95%98%EA%B8%B0-reviewer-%ED%95%B4-%EC%A4%98)
  * [수정할 파일들](#%EC%88%98%EC%A0%95%ED%95%A0-%ED%8C%8C%EC%9D%BC%EB%93%A4)
  * [추가 사항](#%EC%B6%94%EA%B0%80-%EC%82%AC%ED%95%AD)
  * [검증 체크리스트](#%EA%B2%80%EC%A6%9D-%EC%B2%B4%ED%81%AC%EB%A6%AC%EC%8A%A4%ED%8A%B8)
  * [에러 흐름 이해하기](#%EC%97%90%EB%9F%AC-%ED%9D%90%EB%A6%84-%EC%9D%B4%ED%95%B4%ED%95%98%EA%B8%B0)
- [굳이 안봐도 되는 걍 팁들](#%EA%B5%B3%EC%9D%B4-%EC%95%88%EB%B4%90%EB%8F%84-%EB%90%98%EB%8A%94-%EA%B1%8D-%ED%8C%81%EB%93%A4)
  * [흔한 함정들](#%ED%9D%94%ED%95%9C-%ED%95%A8%EC%A0%95%EB%93%A4)
  * [작업하면서 고려할 질문들](#%EC%9E%91%EC%97%85%ED%95%98%EB%A9%B4%EC%84%9C-%EA%B3%A0%EB%A0%A4%ED%95%A0-%EC%A7%88%EB%AC%B8%EB%93%A4)

<!-- tocstop -->

## Rust 기초 입문

### 알아야 할 Rust 기본 개념
- **소유권(Ownership)**: Rust는 데이터를 누가 "소유"하는지 추적합니다. 함수에 데이터를 전달하면 소유권이 이동하는데, `&`(빌림)을 사용하면 이동하지 않습니다.
- **구조체(Structs)**: TypeScript 인터페이스나 Python 클래스와 비슷하지만, 상속은 없습니다. 예시:
  ```rust
  pub struct User {
      pub id: LazySnowflake,
      pub name: String,  // ← 여기를 바꾸게 됩니다!
  }
  ```
- **트레이트(Traits)**: TypeScript 인터페이스나 Python 프로토콜과 유사 - 행동 규약을 정의합니다.
- **Option<T>**: TypeScript의 `T | null`이나 Python의 `Optional[T]`와 같습니다. `Some(value)` 또는 `None`을 사용합니다.
- **Result<T, E>**: 실패할 수 있는 함수에 사용합니다. `Ok(value)` 또는 `Err(error)`를 사용합니다.
- **매크로(Macros)**: 코드를 생성하는 코드로, `!`로 끝납니다. 예: `println!()`, `sqlx::query!()`

### 이 코드베이스의 일반적인 패턴
- `pub` = public (다른 모듈에서 접근 가능)
- `async fn` = 비동기 함수 (TypeScript의 `async function`과 같음)
- `.await?` = 결과를 await하고 에러를 위로 전파 (TypeScript의 `await` + `throw`)
- `&self` = self를 빌림 (Python의 `self`와 유사)
- `String` vs `&str`: `String`은 소유, `&str`은 빌림(참조와 같음)

## HQ 코드베이스 아키텍처

HQ는 **클린 아키텍처**(Reviewer: 안깨끗해 ㅅㅂ) 패턴을 따르며, 관심사를 명확히 분리합니다:

```
src/
├── feature/          # 도메인별로 구성된 비즈니스 로직
│   ├── user/         # ← 여기서 작업하게 됩니다
│   │   ├── mod.rs    # User 구조체 정의
│   │   ├── types.rs  # 요청/응답 DTO (CreateUser, UpdateUserInfo 등)
│   │   ├── service.rs    # 비즈니스 로직 레이어
│   │   ├── repository.rs # 데이터 접근 트레이트 (인터페이스)
│   │   └── error.rs  # 도메인별 에러
│   ├── auth/
│   └── ...
├── infrastructure/   # 리포지토리의 구체적인 구현
│   ├── postgres/
│   │   └── user.rs   # ← User를 위한 SQL 쿼리
│   └── redis/
├── controller/       # HTTP 레이어 (API 라우트)
│   └── routes/
│       └── user.rs   # ← User API 엔드포인트
├── core/             # 앱 초기화 및 의존성 주입
└── util/             # 공유 유틸리티
```

### 주요 아키텍처 개념

**1. 기능 모듈** (`feature/user/`)
- **Types** (`types.rs`): API 요청을 위한 DTO (CreateUser, UpdateUserInfo)
- **Service** (`service.rs`): 비즈니스 로직 조율
- **Repository** (`repository.rs`): 데이터 접근 인터페이스를 정의하는 트레이트 (TS 인터페이스와 유사)
- **Errors** (`error.rs`): 도메인별 에러 타입

**2. 인프라 레이어** (`infrastructure/postgres/user.rs`)
- 리포지토리 트레이트를 구현
- 실제 SQL 쿼리 포함
- 데이터베이스 특화 로직과 에러 매핑 처리

**3. 컨트롤러 레이어** (`controller/routes/user.rs`)
- HTTP 엔드포인트 핸들러
- `axum` 웹 프레임워크 사용 (Node의 Express.js와 유사)
- 권한 검증 및 서비스 호출

**4. 의존성 주입** (`core/app.rs`)
- 서비스와 의존성을 연결
- AppState가 설정과 서비스 인스턴스를 보관

### 데이터 흐름

```
HTTP 요청 → 컨트롤러 → 서비스 → 리포지토리 → 데이터베이스
                ↓         ↓         ↓
             (전달)  (검증/로직) (SQL 쿼리)
```

예시: 사용자 생성
1. `POST /api/v1/user` → `controller/routes/user.rs::create_user()`
2. 컨트롤러가 권한 검증
3. `service.user_service.create_user(CreateUser)` 호출
4. 서비스가 ID 생성 및 User 구조체 생성
5. `user_repo.create_user(User)` 호출 (트레이트 메서드)
6. PostgresDb가 SQL INSERT로 구현
7. 응답이 위로 전달됨

## 일: `name` → `username` 변경하기 (Reviewer: 해 줘!)

User 기능 전체에서 `name` 필드를 `username`으로 변경해야 합니다. 업데이트할 내용은 다음과 같습니다:

### 수정할 파일들

**1. 핵심 타입 정의**
- `src/feature/user.rs`: User 구조체 자체 (14번째 줄)

**2. 데이터베이스 레이어**
- `migrations/20251003225658_user.up.sql`: 데이터베이스 스키마
- `src/infrastructure/postgres/user.rs`: SQL 쿼리 및 컬럼 이름

**3. 비즈니스 로직**
- `src/feature/user/types.rs`: CreateUser와 UpdateUserInfo DTO
- `src/feature/user/service.rs`: 필드를 사용하는 서비스 로직
- `src/feature/user/repository.rs`: UpdateUser 구조체

**4. 테스트**
- `tests/user.rs`: 테스트 코드 바꿔줘

### 추가 사항
에러 매핑 업데이트. 이거때문에 본인 삽질 좀 함.
`<테이블명>_<필드명>_key` 형식이다. `users` 테이블에 `name` 필드면 `users_username_key`

```rust
// map_user_query_error에서:
if pg_err.constraint() == Some("users_username_key") {  // 'users_name_key'였음
    return AppError::Business(BusinessError::User(UserError::DuplicateName));
}
```

### 검증 체크리스트

변경 후:

1. **프로젝트 빌드**: `cargo build`
   - 타입 에러와 컴파일 이슈를 확인합니다

2. **테스트 실행**: `cargo test`
   - 변경사항이 제대로 작동하는지 확인합니다

글고 `cargo build` 에서 노란줄 뜬거 개발할땐 무시해도 되는데 푸시할땐 ㄴㄴ. 깃헙쪽에서 검증하는애는 노란줄도 실패로 그어버리더라.


### 에러 흐름 이해하기

사용자가 중복된 username을 만들려고 할 때:
```
1. PostgresDb.create_user()가 SQL INSERT 실행
2. 데이터베이스가 거부 (unique 제약조건 위반)
3. map_user_query_error()가 에러를 잡음
4. 제약조건 이름 확인: "users_username_key"  <- 이거 빼먹지 않기
5. UserError::DuplicateName 반환
6. 컨트롤러가 400 Bad Request 반환
```

## 굳이 안봐도 되는 걍 팁들

### 흔한 함정들

1. **데이터베이스 제약조건 이름을 잊어버림**: SQL 에러 처리의 unique 제약조건 이름
2. **검증 함수 이름**: `validate_username` 함수는 그대로 두어도 됩니다
3. **SQL 컬럼 이름**: 데이터베이스 스키마와 정확히 일치해야 합니다
4. **Option vs 직접 필드**: `UpdateUser`는 `Option<String>`을 사용하지만, `User`는 `String`을 사용합니다

### 작업하면서 고려할 질문들

1. 왜 `User`는 `String`을 갖고 `UpdateUser`는 `Option<String>`을 가질까요?
   - `User`는 항상 username을 가지지만, 업데이트할 때는 변경하지 않을 수도 있습니다

2. 왜 `repository.rs` (트레이트)와 `postgres/user.rs` (구현)를 분리할까요?
   - 데이터베이스 구현을 교체할 수 있게 합니다 (예: MockUserRepository로 테스트)
