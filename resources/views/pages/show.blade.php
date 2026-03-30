@extends('layouts.tri')

@push('social-meta')
    <meta property="og:description" content="{{ Str::limit($page->text, 100, '...') }}">
@endpush

@include('entities.body-tag-classes', ['entity' => $page])

@section('body')

    <div class="mb-m print-hidden">
        @include('entities.breadcrumbs', ['crumbs' => [
            $page->book,
            $page->hasChapter() ? $page->chapter : null,
            $page,
        ]])
    </div>

    <main class="content-wrap card">
        @if ($commentTree->enabled())
        <div class="page-content-with-gutter"
             x-data="commentGutter({ canComment: {{ userCan(\BookStack\Permissions\Permission::CommentCreateAll) ? 'true' : 'false' }} })">
        @endif
        <div component="page-display"
             option:page-display:page-id="{{ $page->id }}"
             class="page-content clearfix">
            @include('pages.parts.page-display')
        </div>
        @if ($commentTree->enabled())
        </div>
        @endif
        @include('pages.parts.pointer', ['page' => $page, 'commentTree' => $commentTree])
    </main>

    @include('entities.sibling-navigation', ['next' => $next, 'previous' => $previous])

    @if ($commentTree->enabled())
        <div class="comments-container mb-l print-hidden">
            @include('comments.comments', ['commentTree' => $commentTree, 'page' => $page])
            <div class="clearfix"></div>
        </div>
    @endif
@stop

@section('left')
    @include('pages.parts.show-sidebar-section-tags', ['page' => $page])
    @include('pages.parts.show-sidebar-section-attachments', ['page' => $page])
    @include('pages.parts.show-sidebar-section-page-nav', ['pageNav' => $pageNav])
    @include('entities.book-tree', ['book' => $book, 'sidebarTree' => $sidebarTree])
@stop

@section('right')
    @include('pages.parts.show-sidebar-section-details', ['page' => $page, 'watchOptions' => $watchOptions, 'book' => $book])
    @include('pages.parts.show-sidebar-section-actions', ['page' => $page, 'watchOptions' => $watchOptions])
    @include('pages.parts.show-sidebar-section-scratch-notes', ['page' => $page])
    @include('entities.show-sidebar-section-backlinks', ['entity' => $page])
@stop
